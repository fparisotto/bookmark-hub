#![cfg(feature = "integration-tests")]

mod common;

use chrono::{Duration, Utc};
use common::test_db::{create_test_user, TestDatabase};
use server::db::bookmark_task;
use shared::{BookmarkTaskSearchRequest, BookmarkTaskStatus};
use url::Url;
use uuid::Uuid;

#[tokio::test]
async fn test_task_create_and_retrieve() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let url = Url::parse("https://example.com/article")?;
    let tags = vec!["rust".to_string(), "programming".to_string()];

    let task = bookmark_task::create(&db.pool, user_id, url.clone(), tags.clone()).await?;

    // Verify task fields
    assert_eq!(task.user_id, user_id);
    assert_eq!(task.url, url.to_string());
    assert_eq!(task.status, BookmarkTaskStatus::Pending);
    assert_eq!(task.tags, Some(tags));
    assert!(task.summary.is_none());
    assert!(task.retries.is_none());
    assert!(task.fail_reason.is_none());
    assert!(!task.task_id.is_nil());
    assert!(task.created_at <= task.updated_at);
    assert!(task.next_delivery >= task.created_at);

    Ok(())
}

#[tokio::test]
async fn test_peek_tasks() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let now = Utc::now();

    // Create tasks with different URLs
    let urls = vec![
        "https://example.com/1",
        "https://example.com/2",
        "https://example.com/3",
    ];

    let mut created_tasks = Vec::new();
    for url_str in &urls {
        let url = Url::parse(url_str)?;
        let task = bookmark_task::create(&db.pool, user_id, url, vec![]).await?;
        created_tasks.push(task);
    }

    // Peek tasks - should get all pending tasks
    let peeked_tasks = bookmark_task::peek(&db.pool, now + Duration::seconds(1)).await?;

    assert_eq!(peeked_tasks.len(), 3);

    // Verify task IDs match
    let peeked_ids: Vec<Uuid> = peeked_tasks.iter().map(|t| t.task_id).collect();
    for task in &created_tasks {
        assert!(peeked_ids.contains(&task.task_id));
    }

    // Peek again immediately - should get no tasks (next_delivery updated)
    let peeked_again = bookmark_task::peek(&db.pool, now + Duration::seconds(1)).await?;
    assert_eq!(
        peeked_again.len(),
        0,
        "Tasks should not be available immediately after peek"
    );

    // Peek after 5+ minutes - tasks should be available again
    let future_time = now + Duration::minutes(6);
    let peeked_future = bookmark_task::peek(&db.pool, future_time).await?;
    assert_eq!(
        peeked_future.len(),
        3,
        "Tasks should be available after delivery window"
    );

    Ok(())
}

#[tokio::test]
async fn test_peek_limit() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create 15 tasks (more than the limit of 10)
    for i in 0..15 {
        let url = Url::parse(&format!("https://example.com/{}", i))?;
        bookmark_task::create(&db.pool, user_id, url, vec![]).await?;
    }

    // Peek should return at most 10 tasks
    let peeked = bookmark_task::peek(&db.pool, Utc::now() + Duration::seconds(1)).await?;
    assert_eq!(peeked.len(), 10, "Peek should respect the limit of 10");

    Ok(())
}

#[tokio::test]
async fn test_update_task_status() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let url = Url::parse("https://example.com/test")?;
    let task = bookmark_task::create(&db.pool, user_id, url, vec![]).await?;

    // Update to Done status
    bookmark_task::update(&db.pool, task.clone(), BookmarkTaskStatus::Done, None, None).await?;

    // Search for the task to verify update
    let search_req = BookmarkTaskSearchRequest {
        status: Some(BookmarkTaskStatus::Done),
        ..Default::default()
    };
    let search_result = bookmark_task::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(search_result.tasks.len(), 1);
    assert_eq!(search_result.tasks[0].status, BookmarkTaskStatus::Done);

    // Create another task and update to Fail with retry info
    let url2 = Url::parse("https://example.com/fail")?;
    let task2 = bookmark_task::create(&db.pool, user_id, url2, vec![]).await?;

    bookmark_task::update(
        &db.pool,
        task2.clone(),
        BookmarkTaskStatus::Fail,
        Some(3),
        Some("Connection timeout".to_string()),
    )
    .await?;

    let search_req2 = BookmarkTaskSearchRequest {
        status: Some(BookmarkTaskStatus::Fail),
        ..Default::default()
    };
    let search_result2 = bookmark_task::search(&db.pool, user_id, &search_req2).await?;

    assert_eq!(search_result2.tasks.len(), 1);
    assert_eq!(search_result2.tasks[0].status, BookmarkTaskStatus::Fail);
    assert_eq!(search_result2.tasks[0].retries, Some(3));
    assert_eq!(
        search_result2.tasks[0].fail_reason,
        Some("Connection timeout".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_search_by_url() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create tasks with different URLs
    let urls = vec![
        "https://rust-lang.org/learn",
        "https://rust-lang.org/book",
        "https://example.com/other",
    ];

    for url_str in &urls {
        let url = Url::parse(url_str)?;
        bookmark_task::create(&db.pool, user_id, url, vec![]).await?;
    }

    // Search for rust-lang URLs
    let search_req = BookmarkTaskSearchRequest {
        url: Some("rust-lang".to_string()),
        ..Default::default()
    };

    let result = bookmark_task::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.tasks.len(), 2);
    for task in &result.tasks {
        assert!(task.url.contains("rust-lang"));
    }

    Ok(())
}

#[tokio::test]
async fn test_search_by_status() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create tasks with different statuses
    let url1 = Url::parse("https://example.com/1")?;
    let task1 = bookmark_task::create(&db.pool, user_id, url1, vec![]).await?;

    let url2 = Url::parse("https://example.com/2")?;
    let task2 = bookmark_task::create(&db.pool, user_id, url2, vec![]).await?;

    let url3 = Url::parse("https://example.com/3")?;
    let _task3 = bookmark_task::create(&db.pool, user_id, url3, vec![]).await?;

    // Update tasks to different statuses
    bookmark_task::update(&db.pool, task1, BookmarkTaskStatus::Done, None, None).await?;
    bookmark_task::update(
        &db.pool,
        task2,
        BookmarkTaskStatus::Fail,
        Some(1),
        Some("Error".to_string()),
    )
    .await?;
    // task3 remains Pending

    // Search for Done tasks
    let search_done = BookmarkTaskSearchRequest {
        status: Some(BookmarkTaskStatus::Done),
        ..Default::default()
    };
    let result_done = bookmark_task::search(&db.pool, user_id, &search_done).await?;
    assert_eq!(result_done.tasks.len(), 1);
    assert_eq!(result_done.tasks[0].status, BookmarkTaskStatus::Done);

    // Search for Failed tasks
    let search_fail = BookmarkTaskSearchRequest {
        status: Some(BookmarkTaskStatus::Fail),
        ..Default::default()
    };
    let result_fail = bookmark_task::search(&db.pool, user_id, &search_fail).await?;
    assert_eq!(result_fail.tasks.len(), 1);
    assert_eq!(result_fail.tasks[0].status, BookmarkTaskStatus::Fail);

    // Search for Pending tasks
    let search_pending = BookmarkTaskSearchRequest {
        status: Some(BookmarkTaskStatus::Pending),
        ..Default::default()
    };
    let result_pending = bookmark_task::search(&db.pool, user_id, &search_pending).await?;
    assert_eq!(result_pending.tasks.len(), 1);
    assert_eq!(result_pending.tasks[0].status, BookmarkTaskStatus::Pending);

    Ok(())
}

#[tokio::test]
async fn test_search_by_tags() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create tasks with different tag combinations
    let url1 = Url::parse("https://example.com/1")?;
    bookmark_task::create(
        &db.pool,
        user_id,
        url1,
        vec!["rust".to_string(), "async".to_string()],
    )
    .await?;

    let url2 = Url::parse("https://example.com/2")?;
    bookmark_task::create(
        &db.pool,
        user_id,
        url2,
        vec!["rust".to_string(), "web".to_string()],
    )
    .await?;

    let url3 = Url::parse("https://example.com/3")?;
    bookmark_task::create(&db.pool, user_id, url3, vec!["javascript".to_string()]).await?;

    // Search for tasks with "rust" tag
    let search_rust = BookmarkTaskSearchRequest {
        tags: Some(vec!["rust".to_string()]),
        ..Default::default()
    };
    let result_rust = bookmark_task::search(&db.pool, user_id, &search_rust).await?;
    assert_eq!(result_rust.tasks.len(), 2);

    // Search for tasks with both "rust" and "async" tags
    let search_both = BookmarkTaskSearchRequest {
        tags: Some(vec!["rust".to_string(), "async".to_string()]),
        ..Default::default()
    };
    let result_both = bookmark_task::search(&db.pool, user_id, &search_both).await?;
    assert_eq!(result_both.tasks.len(), 1);

    // Search for "javascript" tag
    let search_js = BookmarkTaskSearchRequest {
        tags: Some(vec!["javascript".to_string()]),
        ..Default::default()
    };
    let result_js = bookmark_task::search(&db.pool, user_id, &search_js).await?;
    assert_eq!(result_js.tasks.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_search_by_date_range() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let now = Utc::now();

    // Create tasks at different times
    let url1 = Url::parse("https://example.com/old")?;
    bookmark_task::create(&db.pool, user_id, url1, vec![]).await?;

    // Note: In real tests, you might need to manipulate created_at in the database
    // For this test, we'll use the current time as reference

    // Search for tasks created after a specific time
    let search_from = BookmarkTaskSearchRequest {
        from_created_at: Some(now - Duration::hours(1)),
        ..Default::default()
    };
    let result_from = bookmark_task::search(&db.pool, user_id, &search_from).await?;
    assert!(result_from.tasks.len() > 0);

    // Search for tasks created before a future time
    let search_to = BookmarkTaskSearchRequest {
        to_created_at: Some(now + Duration::hours(1)),
        ..Default::default()
    };
    let result_to = bookmark_task::search(&db.pool, user_id, &search_to).await?;
    assert!(result_to.tasks.len() > 0);

    // Search with both from and to (window)
    let search_window = BookmarkTaskSearchRequest {
        from_created_at: Some(now - Duration::hours(1)),
        to_created_at: Some(now + Duration::hours(1)),
        ..Default::default()
    };
    let result_window = bookmark_task::search(&db.pool, user_id, &search_window).await?;
    assert!(result_window.tasks.len() > 0);

    Ok(())
}

#[tokio::test]
async fn test_search_pagination() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create 10 tasks
    let mut all_task_ids = Vec::new();
    for i in 0..10 {
        let url = Url::parse(&format!("https://example.com/{}", i))?;
        let task = bookmark_task::create(&db.pool, user_id, url, vec![]).await?;
        all_task_ids.push(task.task_id);
    }

    // Sort task IDs for comparison (search orders by task_id ASC)
    all_task_ids.sort();

    // First page with page_size = 3
    let page1_req = BookmarkTaskSearchRequest {
        page_size: Some(3),
        ..Default::default()
    };
    let page1 = bookmark_task::search(&db.pool, user_id, &page1_req).await?;

    assert_eq!(page1.tasks.len(), 3);
    assert!(page1.has_more);

    // Second page using last_task_id
    let last_id = page1.tasks.last().unwrap().task_id;
    let page2_req = BookmarkTaskSearchRequest {
        page_size: Some(3),
        last_task_id: Some(last_id),
        ..Default::default()
    };
    let page2 = bookmark_task::search(&db.pool, user_id, &page2_req).await?;

    assert_eq!(page2.tasks.len(), 3);
    assert!(page2.has_more);

    // Verify no overlap between pages
    let page1_ids: Vec<Uuid> = page1.tasks.iter().map(|t| t.task_id).collect();
    let page2_ids: Vec<Uuid> = page2.tasks.iter().map(|t| t.task_id).collect();
    for id in &page1_ids {
        assert!(!page2_ids.contains(id), "Pages should not overlap");
    }

    // Third page
    let last_id2 = page2.tasks.last().unwrap().task_id;
    let page3_req = BookmarkTaskSearchRequest {
        page_size: Some(3),
        last_task_id: Some(last_id2),
        ..Default::default()
    };
    let page3 = bookmark_task::search(&db.pool, user_id, &page3_req).await?;

    assert_eq!(page3.tasks.len(), 3);
    assert!(page3.has_more);

    // Fourth page (last page with only 1 item)
    let last_id3 = page3.tasks.last().unwrap().task_id;
    let page4_req = BookmarkTaskSearchRequest {
        page_size: Some(3),
        last_task_id: Some(last_id3),
        ..Default::default()
    };
    let page4 = bookmark_task::search(&db.pool, user_id, &page4_req).await?;

    assert_eq!(page4.tasks.len(), 1);
    assert!(!page4.has_more, "Last page should have has_more = false");

    Ok(())
}

#[tokio::test]
async fn test_search_combined_filters() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create tasks with various combinations
    let url1 = Url::parse("https://rust-lang.org/learn")?;
    bookmark_task::create(
        &db.pool,
        user_id,
        url1,
        vec!["rust".to_string(), "tutorial".to_string()],
    )
    .await?;

    let url2 = Url::parse("https://rust-lang.org/book")?;
    let task2 = bookmark_task::create(
        &db.pool,
        user_id,
        url2,
        vec!["rust".to_string(), "book".to_string()],
    )
    .await?;

    let url3 = Url::parse("https://example.com/rust")?;
    bookmark_task::create(&db.pool, user_id, url3, vec!["rust".to_string()]).await?;

    // Update one task to Done
    bookmark_task::update(&db.pool, task2, BookmarkTaskStatus::Done, None, None).await?;

    // Search with multiple filters: URL pattern + tags + status
    let search_req = BookmarkTaskSearchRequest {
        url: Some("rust-lang".to_string()),
        tags: Some(vec!["rust".to_string()]),
        status: Some(BookmarkTaskStatus::Pending),
        page_size: Some(10),
        ..Default::default()
    };

    let result = bookmark_task::search(&db.pool, user_id, &search_req).await?;

    // Should find only the first task (rust-lang URL, has "rust" tag, and is
    // Pending)
    assert_eq!(result.tasks.len(), 1);
    assert!(result.tasks[0].url.contains("rust-lang.org/learn"));

    Ok(())
}

#[tokio::test]
async fn test_concurrent_peek() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create 5 tasks
    for i in 0..5 {
        let url = Url::parse(&format!("https://example.com/{}", i))?;
        bookmark_task::create(&db.pool, user_id, url, vec![]).await?;
    }

    let now = Utc::now() + Duration::seconds(1);

    // Simulate concurrent peeks
    let pool1 = db.pool.clone();
    let pool2 = db.pool.clone();

    let handle1 = tokio::spawn(async move { bookmark_task::peek(&pool1, now).await });

    let handle2 = tokio::spawn(async move { bookmark_task::peek(&pool2, now).await });

    let result1 = handle1.await??;
    let result2 = handle2.await??;

    // Verify no task appears in both results (FOR UPDATE SKIP LOCKED should prevent
    // this)
    let ids1: Vec<Uuid> = result1.iter().map(|t| t.task_id).collect();
    let ids2: Vec<Uuid> = result2.iter().map(|t| t.task_id).collect();

    for id in &ids1 {
        assert!(
            !ids2.contains(id),
            "Task should not be peeked by both operations"
        );
    }

    // Total should be 5 tasks
    assert_eq!(
        ids1.len() + ids2.len(),
        5,
        "All tasks should be peeked exactly once"
    );

    Ok(())
}

#[tokio::test]
async fn test_user_isolation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Create two users
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    // Create tasks for each user
    let url1 = Url::parse("https://example.com/user1")?;
    bookmark_task::create(&db.pool, user1_id, url1, vec!["user1".to_string()]).await?;

    let url2 = Url::parse("https://example.com/user2")?;
    bookmark_task::create(&db.pool, user2_id, url2, vec!["user2".to_string()]).await?;

    // Search as user1 - should only see user1's tasks
    let search_req = BookmarkTaskSearchRequest::default();
    let result1 = bookmark_task::search(&db.pool, user1_id, &search_req).await?;

    assert_eq!(result1.tasks.len(), 1);
    assert_eq!(result1.tasks[0].user_id, user1_id);
    assert!(result1.tasks[0]
        .tags
        .as_ref()
        .unwrap()
        .contains(&"user1".to_string()));

    // Search as user2 - should only see user2's tasks
    let result2 = bookmark_task::search(&db.pool, user2_id, &search_req).await?;

    assert_eq!(result2.tasks.len(), 1);
    assert_eq!(result2.tasks[0].user_id, user2_id);
    assert!(result2.tasks[0]
        .tags
        .as_ref()
        .unwrap()
        .contains(&"user2".to_string()));

    Ok(())
}
