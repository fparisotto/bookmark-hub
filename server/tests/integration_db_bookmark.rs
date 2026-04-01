#![cfg(feature = "integration-tests")]

mod common;

use chrono::{Duration, Utc};
use common::test_db::{create_test_bookmark, create_test_user, TestDatabase};
use server::db::bookmark::{self, AiGenerationStatus};
use shared::TagOperation;

async fn get_processing_state(
    db: &TestDatabase,
    bookmark_id: &str,
) -> anyhow::Result<(
    AiGenerationStatus,
    i16,
    Option<String>,
    AiGenerationStatus,
    i16,
    Option<String>,
)> {
    let client = db.pool.get().await?;
    let row = client
        .query_one(
            "SELECT summary_status, summary_attempts, summary_fail_reason, tag_status, tag_attempts, tag_fail_reason
             FROM bookmark
             WHERE bookmark_id = $1",
            &[&bookmark_id],
        )
        .await?;

    Ok((
        row.get("summary_status"),
        row.get("summary_attempts"),
        row.get("summary_fail_reason"),
        row.get("tag_status"),
        row.get("tag_attempts"),
        row.get("tag_fail_reason"),
    ))
}

#[tokio::test]
async fn test_bookmark_save_and_retrieve() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/article",
        "Example Article",
        "example.com",
        None,
    );

    let text_content = "This is the full text content of the article.";

    // Save bookmark
    let saved_bookmark = bookmark::save(&db.pool, &bookmark, text_content).await?;

    // Verify saved fields
    assert_eq!(saved_bookmark.bookmark_id, bookmark.bookmark_id);
    assert_eq!(saved_bookmark.user_id, user_id);
    assert_eq!(saved_bookmark.url, bookmark.url);
    assert_eq!(saved_bookmark.domain, bookmark.domain);
    assert_eq!(saved_bookmark.title, bookmark.title);
    assert_eq!(saved_bookmark.tags, None);
    assert_eq!(saved_bookmark.summary, None);
    assert!(saved_bookmark.created_at <= saved_bookmark.updated_at.unwrap());

    // Test canonical URL lookup
    let retrieved =
        bookmark::get_by_canonical_url_and_user_id(&db.pool, &bookmark.url, user_id).await?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.bookmark_id, bookmark.bookmark_id);

    // Test get_with_user_data
    let retrieved_by_id =
        bookmark::get_with_user_data(&db.pool, user_id, &bookmark.bookmark_id).await?;
    assert!(retrieved_by_id.is_some());
    assert_eq!(retrieved_by_id.unwrap().bookmark_id, bookmark.bookmark_id);

    // Test get_text_content
    let text = bookmark::get_text_content(&db.pool, user_id, &bookmark.bookmark_id).await?;
    assert!(text.is_some());
    assert_eq!(text.unwrap(), text_content);

    Ok(())
}

#[tokio::test]
async fn test_get_by_user() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create multiple bookmarks
    let bookmarks_data = vec![
        ("https://example.com/1", "Article 1", "example.com"),
        ("https://rust-lang.org", "Rust Lang", "rust-lang.org"),
        ("https://github.com/rust", "Rust on GitHub", "github.com"),
    ];

    let mut saved_bookmark_ids = Vec::new();
    for (url, title, domain) in &bookmarks_data {
        let bookmark = create_test_bookmark(user_id, url, title, domain, None);
        let saved = bookmark::save(&db.pool, &bookmark, "content").await?;
        saved_bookmark_ids.push(saved.bookmark_id);
    }

    // Get all bookmarks for user
    let user_bookmarks = bookmark::get_by_user(&db.pool, user_id).await?;

    assert_eq!(user_bookmarks.len(), 3);

    // Verify all bookmarks are returned and ordered by created_at ASC
    for (i, bookmark) in user_bookmarks.iter().enumerate() {
        assert_eq!(bookmark.user_id, user_id);
        assert!(saved_bookmark_ids.contains(&bookmark.bookmark_id));
        if i > 0 {
            assert!(bookmark.created_at >= user_bookmarks[i - 1].created_at);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_get_by_tag() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different tag combinations
    let mut bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/rust",
        "Rust Article",
        "example.com",
        None,
    );
    bookmark1.tags = Some(vec!["rust".to_string(), "programming".to_string()]);
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let mut bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/web",
        "Web Article",
        "example.com",
        None,
    );
    bookmark2.tags = Some(vec!["rust".to_string(), "web".to_string()]);
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    let mut bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/js",
        "JS Article",
        "example.com",
        None,
    );
    bookmark3.tags = Some(vec!["javascript".to_string()]);
    bookmark::save(&db.pool, &bookmark3, "content3").await?;

    // Search for "rust" tag
    let rust_bookmarks = bookmark::get_by_tag(&db.pool, user_id, "rust").await?;
    assert_eq!(rust_bookmarks.len(), 2);
    for bookmark in &rust_bookmarks {
        assert!(bookmark
            .tags
            .as_ref()
            .unwrap()
            .contains(&"rust".to_string()));
    }

    // Search for "web" tag
    let web_bookmarks = bookmark::get_by_tag(&db.pool, user_id, "web").await?;
    assert_eq!(web_bookmarks.len(), 1);
    assert_eq!(web_bookmarks[0].url, "https://example.com/web");

    // Search for "javascript" tag
    let js_bookmarks = bookmark::get_by_tag(&db.pool, user_id, "javascript").await?;
    assert_eq!(js_bookmarks.len(), 1);
    assert_eq!(js_bookmarks[0].url, "https://example.com/js");

    // Search for non-existent tag
    let none_bookmarks = bookmark::get_by_tag(&db.pool, user_id, "nonexistent").await?;
    assert_eq!(none_bookmarks.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_get_by_canonical_url_and_user_id() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    let bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/shared?a=1",
        "Shared URL",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/shared?a=1",
        "Same URL Different User",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    // Get bookmark for user1
    let result1 = bookmark::get_by_canonical_url_and_user_id(
        &db.pool,
        "https://EXAMPLE.com:443/shared?a=1#top",
        user1_id,
    )
    .await?;
    assert!(result1.is_some());
    assert_eq!(result1.unwrap().bookmark_id, bookmark1.bookmark_id);

    // Get bookmark for user2
    let result2 = bookmark::get_by_canonical_url_and_user_id(
        &db.pool,
        "https://example.com/shared?a=1",
        user2_id,
    )
    .await?;
    assert!(result2.is_some());
    assert_eq!(result2.unwrap().bookmark_id, bookmark2.bookmark_id);

    // Try to get non-existent URL
    let result3 =
        bookmark::get_by_canonical_url_and_user_id(&db.pool, "https://nonexistent.com", user1_id)
            .await?;
    assert!(result3.is_none());

    Ok(())
}

#[tokio::test]
async fn test_same_bookmark_id_can_exist_for_different_users() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    let mut bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/user-1",
        "User1 Article",
        "example.com",
        None,
    );
    bookmark1.bookmark_id = "shared-bookmark-id".to_string();
    let saved1 = bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let mut bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/user-2",
        "User2 Article",
        "example.com",
        None,
    );
    bookmark2.bookmark_id = "shared-bookmark-id".to_string();
    let saved2 = bookmark::save(&db.pool, &bookmark2, "content2").await?;

    assert_eq!(saved1.bookmark_id, saved2.bookmark_id);
    assert_ne!(saved1.user_id, saved2.user_id);

    Ok(())
}

#[tokio::test]
async fn test_get_with_user_data() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    let bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/user1",
        "User1 Article",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    // User1 should be able to get their bookmark
    let result1 = bookmark::get_with_user_data(&db.pool, user1_id, &bookmark1.bookmark_id).await?;
    assert!(result1.is_some());
    assert_eq!(result1.unwrap().bookmark_id, bookmark1.bookmark_id);

    // User2 should NOT be able to get user1's bookmark
    let result2 = bookmark::get_with_user_data(&db.pool, user2_id, &bookmark1.bookmark_id).await?;
    assert!(result2.is_none());

    // Non-existent bookmark should return None
    let result3 = bookmark::get_with_user_data(&db.pool, user1_id, "nonexistent_id").await?;
    assert!(result3.is_none());

    Ok(())
}

#[tokio::test]
async fn test_update_tags_set() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/tags",
        "Tags Article",
        "example.com",
        Some(vec!["initial".to_string()]),
    );
    let saved = bookmark::save(&db.pool, &bookmark, "content").await?;

    // Set new tags (replace existing)
    let new_tags = vec!["rust".to_string(), "programming".to_string()];
    let tag_operation = TagOperation::Set(new_tags.clone());

    let updated =
        bookmark::update_tags(&db.pool, user_id, &saved.bookmark_id, &tag_operation).await?;

    assert_eq!(updated.tags, Some(new_tags));
    assert!(updated.updated_at.is_some());
    assert!(updated.updated_at > saved.updated_at);

    // Verify in database
    let retrieved = bookmark::get_with_user_data(&db.pool, user_id, &saved.bookmark_id).await?;
    assert!(retrieved.is_some());
    assert_eq!(
        retrieved.unwrap().tags,
        Some(vec!["rust".to_string(), "programming".to_string()])
    );

    let (_, _, _, tag_status, tag_attempts, tag_fail_reason) =
        get_processing_state(&db, &saved.bookmark_id).await?;
    assert_eq!(tag_status, AiGenerationStatus::Done);
    assert_eq!(tag_attempts, 0);
    assert!(tag_fail_reason.is_none());

    Ok(())
}

#[tokio::test]
async fn test_update_tags_append() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/append",
        "Append Tags",
        "example.com",
        Some(vec!["initial".to_string(), "existing".to_string()]),
    );
    let saved = bookmark::save(&db.pool, &bookmark, "content").await?;

    // Append new tags
    let append_tags = vec!["new".to_string(), "additional".to_string()];
    let tag_operation = TagOperation::Append(append_tags.clone());

    let updated =
        bookmark::update_tags(&db.pool, user_id, &saved.bookmark_id, &tag_operation).await?;

    // Should contain both original and new tags (order may vary due to DISTINCT)
    let mut expected_tags = vec![
        "initial".to_string(),
        "existing".to_string(),
        "new".to_string(),
        "additional".to_string(),
    ];
    expected_tags.sort();
    let mut actual_tags = updated.tags.unwrap();
    actual_tags.sort();
    assert_eq!(actual_tags, expected_tags);

    // Verify in database
    let retrieved = bookmark::get_with_user_data(&db.pool, user_id, &saved.bookmark_id).await?;
    assert!(retrieved.is_some());
    let mut retrieved_tags = retrieved.unwrap().tags.unwrap();
    retrieved_tags.sort();
    assert_eq!(retrieved_tags, expected_tags);

    Ok(())
}

#[tokio::test]
async fn test_update_summary() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/summary",
        "Summary Article",
        "example.com",
        None,
    );
    let saved = bookmark::save(&db.pool, &bookmark, "content").await?;

    // Update summary
    let summary_text = "This is a summary of the article content.";
    let updated =
        bookmark::update_summary(&db.pool, user_id, &saved.bookmark_id, summary_text).await?;

    assert_eq!(updated.summary, Some(summary_text.to_string()));
    assert!(updated.updated_at.is_some());
    assert!(updated.updated_at > saved.updated_at);

    // Verify in database
    let retrieved = bookmark::get_with_user_data(&db.pool, user_id, &saved.bookmark_id).await?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().summary, Some(summary_text.to_string()));

    let (summary_status, summary_attempts, summary_fail_reason, _, _, _) =
        get_processing_state(&db, &saved.bookmark_id).await?;
    assert_eq!(summary_status, AiGenerationStatus::Done);
    assert_eq!(summary_attempts, 0);
    assert!(summary_fail_reason.is_none());

    Ok(())
}

#[tokio::test]
async fn test_get_text_content() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/text",
        "Text Article",
        "example.com",
        None,
    );
    let text_content = "This is the extracted text content from the webpage.";
    let saved = bookmark::save(&db.pool, &bookmark, text_content).await?;

    // Get text content
    let retrieved_text = bookmark::get_text_content(&db.pool, user_id, &saved.bookmark_id).await?;
    assert!(retrieved_text.is_some());
    assert_eq!(retrieved_text.unwrap(), text_content);

    // Try to get text content for non-existent bookmark
    let no_text = bookmark::get_text_content(&db.pool, user_id, "nonexistent").await?;
    assert!(no_text.is_none());

    Ok(())
}

#[tokio::test]
async fn test_get_tag_count_by_user() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    // Create bookmarks with various tags for user1
    let bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/1",
        "Article 1",
        "example.com",
        Some(vec!["rust".to_string(), "programming".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let bookmark2 = create_test_bookmark(
        user1_id,
        "https://example.com/2",
        "Article 2",
        "example.com",
        Some(vec!["rust".to_string(), "web".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    let bookmark3 = create_test_bookmark(
        user1_id,
        "https://example.com/3",
        "Article 3",
        "example.com",
        Some(vec!["programming".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark3, "content3").await?;

    // Create bookmark for user2 (should not affect user1's counts)
    let bookmark4 = create_test_bookmark(
        user2_id,
        "https://example.com/4",
        "Article 4",
        "example.com",
        Some(vec!["rust".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark4, "content4").await?;

    // Get tag counts for user1
    let tag_counts = bookmark::get_tag_count_by_user(&db.pool, user1_id).await?;

    // Convert to HashMap for easier testing
    let counts_map: std::collections::HashMap<String, i64> = tag_counts.into_iter().collect();

    assert_eq!(counts_map.get("rust"), Some(&2)); // appears in 2 bookmarks
    assert_eq!(counts_map.get("programming"), Some(&2)); // appears in 2 bookmarks
    assert_eq!(counts_map.get("web"), Some(&1)); // appears in 1 bookmark
    assert!(!counts_map.contains_key("nonexistent")); // should not exist

    // Get tag counts for user2 (should only see their own)
    let user2_counts = bookmark::get_tag_count_by_user(&db.pool, user2_id).await?;
    let user2_map: std::collections::HashMap<String, i64> = user2_counts.into_iter().collect();

    assert_eq!(user2_map.get("rust"), Some(&1)); // only 1 for user2
    assert!(!user2_map.contains_key("programming")); // user2 doesn't have this tag

    Ok(())
}

#[tokio::test]
async fn test_get_bookmarks_pending_tag_generation_respects_backoff() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmark with tags
    let tagged_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/tagged",
        "Tagged",
        "example.com",
        Some(vec!["tag".to_string()]),
    );
    bookmark::save(&db.pool, &tagged_bookmark, "content1").await?;

    // Create bookmark with empty tags array
    let empty_tags_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/empty",
        "Empty Tags",
        "example.com",
        Some(vec![]),
    );
    let empty_saved = bookmark::save(&db.pool, &empty_tags_bookmark, "content2").await?;

    // Create bookmark with null tags
    let null_tags_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/null",
        "Null Tags",
        "example.com",
        None,
    );
    let null_saved = bookmark::save(&db.pool, &null_tags_bookmark, "content3").await?;

    let now = Utc::now() + Duration::seconds(1);
    let pending = bookmark::get_bookmarks_pending_tag_generation(&db.pool, 10, now).await?;
    assert_eq!(pending.len(), 2);
    let urls: Vec<&str> = pending
        .iter()
        .map(|candidate| candidate.bookmark.url.as_str())
        .collect();
    assert!(urls.contains(&"https://example.com/empty"));
    assert!(urls.contains(&"https://example.com/null"));
    assert!(!urls.contains(&"https://example.com/tagged"));

    let retry_at = now + Duration::minutes(5);
    bookmark::mark_tag_generation_failure(
        &db.pool,
        user_id,
        &empty_saved.bookmark_id,
        AiGenerationStatus::Pending,
        1,
        retry_at,
        "temporary failure",
    )
    .await?;

    let pending_before_retry =
        bookmark::get_bookmarks_pending_tag_generation(&db.pool, 10, now).await?;
    assert_eq!(pending_before_retry.len(), 1);
    assert_eq!(
        pending_before_retry[0].bookmark.bookmark_id,
        null_saved.bookmark_id
    );

    let pending_after_retry = bookmark::get_bookmarks_pending_tag_generation(
        &db.pool,
        10,
        retry_at + Duration::seconds(1),
    )
    .await?;
    let retried = pending_after_retry
        .iter()
        .find(|candidate| candidate.bookmark.bookmark_id == empty_saved.bookmark_id)
        .expect("bookmark should be retried after backoff");
    assert_eq!(retried.attempts, 1);

    bookmark::mark_tag_generation_failure(
        &db.pool,
        user_id,
        &empty_saved.bookmark_id,
        AiGenerationStatus::Fail,
        5,
        retry_at,
        "permanent failure",
    )
    .await?;

    let after_terminal_failure =
        bookmark::get_bookmarks_pending_tag_generation(&db.pool, 10, retry_at + Duration::hours(1))
            .await?;
    assert!(after_terminal_failure
        .iter()
        .all(|candidate| candidate.bookmark.bookmark_id != empty_saved.bookmark_id));

    Ok(())
}

#[tokio::test]
async fn test_get_bookmarks_pending_summary_generation_respects_backoff() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmark with summary
    let with_summary = create_test_bookmark(
        user_id,
        "https://example.com/summary",
        "With Summary",
        "example.com",
        None,
    );
    let saved_with_summary = bookmark::save(&db.pool, &with_summary, "content1").await?;
    let _saved_with_summary = bookmark::update_summary(
        &db.pool,
        user_id,
        &saved_with_summary.bookmark_id,
        "This has a summary",
    )
    .await?;

    // Create bookmark without summary
    let without_summary = create_test_bookmark(
        user_id,
        "https://example.com/no-summary",
        "No Summary",
        "example.com",
        None,
    );
    let pending_summary = bookmark::save(&db.pool, &without_summary, "content2").await?;

    let now = Utc::now() + Duration::seconds(1);
    let pending = bookmark::get_bookmarks_pending_summary_generation_at(&db.pool, 10, now).await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].bookmark.bookmark_id, pending_summary.bookmark_id);
    assert_eq!(pending[0].attempts, 0);

    let retry_at = now + Duration::minutes(15);
    bookmark::mark_summary_generation_failure(
        &db.pool,
        user_id,
        &pending_summary.bookmark_id,
        AiGenerationStatus::Pending,
        1,
        retry_at,
        "temporary failure",
    )
    .await?;

    let before_retry =
        bookmark::get_bookmarks_pending_summary_generation_at(&db.pool, 10, now).await?;
    assert!(before_retry.is_empty());

    let after_retry = bookmark::get_bookmarks_pending_summary_generation_at(
        &db.pool,
        10,
        retry_at + Duration::seconds(1),
    )
    .await?;
    assert_eq!(after_retry.len(), 1);
    assert_eq!(after_retry[0].attempts, 1);

    bookmark::mark_summary_generation_failure(
        &db.pool,
        user_id,
        &pending_summary.bookmark_id,
        AiGenerationStatus::Fail,
        5,
        retry_at,
        "permanent failure",
    )
    .await?;

    let after_terminal_failure = bookmark::get_bookmarks_pending_summary_generation_at(
        &db.pool,
        10,
        retry_at + Duration::hours(1),
    )
    .await?;
    assert!(after_terminal_failure.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_user_isolation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    // Create bookmarks for each user
    let bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/user1",
        "User1 Bookmark",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/user2",
        "User2 Bookmark",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    // Test get_by_user isolation
    let user1_bookmarks = bookmark::get_by_user(&db.pool, user1_id).await?;
    assert_eq!(user1_bookmarks.len(), 1);
    assert_eq!(user1_bookmarks[0].user_id, user1_id);

    let user2_bookmarks = bookmark::get_by_user(&db.pool, user2_id).await?;
    assert_eq!(user2_bookmarks.len(), 1);
    assert_eq!(user2_bookmarks[0].user_id, user2_id);

    // Test get_with_user_data isolation (user can't access other user's bookmarks)
    let user1_cant_access =
        bookmark::get_with_user_data(&db.pool, user1_id, &bookmark2.bookmark_id).await?;
    assert!(user1_cant_access.is_none());

    let user2_cant_access =
        bookmark::get_with_user_data(&db.pool, user2_id, &bookmark1.bookmark_id).await?;
    assert!(user2_cant_access.is_none());

    // Test tag count isolation
    let tagged_bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/tagged1",
        "Tagged1",
        "example.com",
        Some(vec!["shared".to_string()]),
    );
    bookmark::save(&db.pool, &tagged_bookmark1, "content").await?;

    let tagged_bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/tagged2",
        "Tagged2",
        "example.com",
        Some(vec!["shared".to_string()]),
    );
    bookmark::save(&db.pool, &tagged_bookmark2, "content").await?;

    let user1_tag_counts = bookmark::get_tag_count_by_user(&db.pool, user1_id).await?;
    let user1_counts: std::collections::HashMap<String, i64> =
        user1_tag_counts.into_iter().collect();
    assert_eq!(user1_counts.get("shared"), Some(&1)); // only user1's bookmark

    let user2_tag_counts = bookmark::get_tag_count_by_user(&db.pool, user2_id).await?;
    let user2_counts: std::collections::HashMap<String, i64> =
        user2_tag_counts.into_iter().collect();
    assert_eq!(user2_counts.get("shared"), Some(&1)); // only user2's bookmark

    Ok(())
}

#[tokio::test]
async fn test_duplicate_url_per_user() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = create_test_user(&db).await?;
    let user2_id = create_test_user(&db).await?;

    let url = "https://example.com/duplicate?a=1";

    // User1 saves URL - should succeed
    let bookmark1 = create_test_bookmark(user1_id, url, "User1 Title", "example.com", None);
    let saved1 = bookmark::save(&db.pool, &bookmark1, "content1").await?;
    assert_eq!(saved1.url, url);

    // User2 saves same URL - should also succeed (different user)
    let bookmark2 = create_test_bookmark(user2_id, url, "User2 Title", "example.com", None);
    let saved2 = bookmark::save(&db.pool, &bookmark2, "content2").await?;
    assert_eq!(saved2.url, url);

    // User1 tries to save the same canonical URL again with a variant raw URL.
    let bookmark1_duplicate = create_test_bookmark(
        user1_id,
        "https://EXAMPLE.com:443/duplicate?a=1#top",
        "Duplicate Title",
        "example.com",
        None,
    );
    let result = bookmark::save(&db.pool, &bookmark1_duplicate, "duplicate content").await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_canonical_url_distinguishes_query_scheme_and_port() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmarks = vec![
        create_test_bookmark(
            user_id,
            "https://example.com/article?a=1",
            "Query One",
            "example.com",
            None,
        ),
        create_test_bookmark(
            user_id,
            "https://example.com/article?a=2",
            "Query Two",
            "example.com",
            None,
        ),
        create_test_bookmark(
            user_id,
            "http://example.com/article?a=1",
            "Http Variant",
            "example.com",
            None,
        ),
        create_test_bookmark(
            user_id,
            "https://example.com:8443/article?a=1",
            "Port Variant",
            "example.com",
            None,
        ),
    ];

    for bookmark_to_save in bookmarks {
        bookmark::save(&db.pool, &bookmark_to_save, "content").await?;
    }

    let user_bookmarks = bookmark::get_by_user(&db.pool, user_id).await?;
    assert_eq!(user_bookmarks.len(), 4);

    Ok(())
}
