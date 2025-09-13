#![cfg(feature = "integration-tests")]

mod common;

use common::test_db::{create_test_bookmark, create_test_user, TestDatabase};
use server::db::{bookmark, search};
use shared::{SearchRequest, TagFilter};

#[tokio::test]
async fn test_basic_search_without_query() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create several bookmarks
    let bookmarks_data = vec![
        (
            "https://example.com/1",
            "First Article",
            "example.com",
            vec!["rust".to_string()],
        ),
        (
            "https://example.com/2",
            "Second Article",
            "example.com",
            vec!["web".to_string()],
        ),
        (
            "https://example.com/3",
            "Third Article",
            "example.com",
            vec!["programming".to_string()],
        ),
    ];

    for (url, title, domain, tags) in &bookmarks_data {
        let bookmark = create_test_bookmark(user_id, url, title, domain, Some(tags.clone()));
        bookmark::save(&db.pool, &bookmark, "Some text content for the article").await?;
    }

    // Search without query (should return all bookmarks)
    let search_req = SearchRequest {
        query: None,
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 3);

    // Verify all bookmarks are returned
    let urls: Vec<&String> = result.items.iter().map(|item| &item.bookmark.url).collect();
    assert!(urls.contains(&&"https://example.com/1".to_string()));
    assert!(urls.contains(&&"https://example.com/2".to_string()));
    assert!(urls.contains(&&"https://example.com/3".to_string()));

    // Verify ordering (should be by created_at DESC when no query)
    assert!(result.items[0].bookmark.created_at >= result.items[1].bookmark.created_at);
    assert!(result.items[1].bookmark.created_at >= result.items[2].bookmark.created_at);

    // Verify tag aggregation
    assert_eq!(result.tags.len(), 3);
    let tag_names: Vec<&String> = result.tags.iter().map(|t| &t.tag).collect();
    assert!(tag_names.contains(&&"rust".to_string()));
    assert!(tag_names.contains(&&"web".to_string()));
    assert!(tag_names.contains(&&"programming".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_full_text_search() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different content
    let bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/rust",
        "Rust Programming Guide",
        "example.com",
        Some(vec!["rust".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark1,
        "Rust is a systems programming language focused on safety and performance",
    )
    .await?;

    let bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/javascript",
        "JavaScript Tutorial",
        "example.com",
        Some(vec!["javascript".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark2,
        "JavaScript is a dynamic programming language for web development",
    )
    .await?;

    let bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/safety",
        "Safety in Programming",
        "example.com",
        Some(vec!["programming".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark3,
        "Programming safety is crucial for building reliable software systems",
    )
    .await?;

    // Search for "rust programming"
    let search_req = SearchRequest {
        query: Some("rust programming".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    // Should find bookmarks containing either "rust" or "programming"
    assert!(result.items.len() >= 1);
    assert!(result.total >= 1);

    // The rust bookmark should be first (higher relevance)
    let first_result = &result.items[0];
    assert!(
        first_result.bookmark.url.contains("rust") || first_result.bookmark.title.contains("Rust")
    );

    // Search for specific term
    let search_req2 = SearchRequest {
        query: Some("javascript".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result2 = search::search(&db.pool, user_id, &search_req2).await?;

    assert_eq!(result2.items.len(), 1);
    assert_eq!(result2.total, 1);
    assert!(result2.items[0].bookmark.url.contains("javascript"));

    Ok(())
}

#[tokio::test]
async fn test_search_highlights() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/highlight",
        "Highlight Test Article",
        "example.com",
        None,
    );
    bookmark::save(
        &db.pool,
        &bookmark,
        "This article discusses the importance of testing and debugging in software development",
    )
    .await?;

    // Search for "testing"
    let search_req = SearchRequest {
        query: Some("testing".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.total, 1);

    // Verify highlight markup is present
    let search_match = &result.items[0].search_match;
    assert!(search_match.is_some());
    let highlight = search_match.as_ref().unwrap();
    assert!(highlight.contains("<mark>"));
    assert!(highlight.contains("</mark>"));
    assert!(highlight.contains("testing") || highlight.contains("Testing"));

    Ok(())
}

#[tokio::test]
async fn test_tag_filter_and() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different tag combinations
    let bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/rust-web",
        "Rust Web Development",
        "example.com",
        Some(vec!["rust".to_string(), "web".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/rust-cli",
        "Rust CLI Tools",
        "example.com",
        Some(vec!["rust".to_string(), "cli".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    let bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/web-only",
        "Web Development",
        "example.com",
        Some(vec!["web".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark3, "content3").await?;

    // Search for bookmarks with both "rust" AND "web" tags
    let search_req = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::And(vec!["rust".to_string(), "web".to_string()])),
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.total, 1);
    assert!(result.items[0].bookmark.url.contains("rust-web"));

    Ok(())
}

#[tokio::test]
async fn test_tag_filter_or() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different tags
    let bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/rust",
        "Rust Guide",
        "example.com",
        Some(vec!["rust".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark1, "content1").await?;

    let bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/python",
        "Python Guide",
        "example.com",
        Some(vec!["python".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark2, "content2").await?;

    let bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/javascript",
        "JavaScript Guide",
        "example.com",
        Some(vec!["javascript".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark3, "content3").await?;

    // Search for bookmarks with either "rust" OR "python" tags
    let search_req = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::Or(vec![
            "rust".to_string(),
            "python".to_string(),
        ])),
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    let urls: Vec<&String> = result.items.iter().map(|item| &item.bookmark.url).collect();
    assert!(urls.contains(&&"https://example.com/rust".to_string()));
    assert!(urls.contains(&&"https://example.com/python".to_string()));
    assert!(!urls.contains(&&"https://example.com/javascript".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_tag_filter_untagged() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with and without tags
    let tagged_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/tagged",
        "Tagged Article",
        "example.com",
        Some(vec!["tag".to_string()]),
    );
    bookmark::save(&db.pool, &tagged_bookmark, "content1").await?;

    let untagged_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/untagged",
        "Untagged Article",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &untagged_bookmark, "content2").await?;

    let empty_tags_bookmark = create_test_bookmark(
        user_id,
        "https://example.com/empty",
        "Empty Tags Article",
        "example.com",
        Some(vec![]),
    );
    bookmark::save(&db.pool, &empty_tags_bookmark, "content3").await?;

    // Search for untagged bookmarks
    let search_req = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::Untagged),
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 2); // null tags and empty array
    assert_eq!(result.total, 2);

    let urls: Vec<&String> = result.items.iter().map(|item| &item.bookmark.url).collect();
    assert!(urls.contains(&&"https://example.com/untagged".to_string()));
    assert!(urls.contains(&&"https://example.com/empty".to_string()));
    assert!(!urls.contains(&&"https://example.com/tagged".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_tag_filter_any() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with various tag states
    let tagged = create_test_bookmark(
        user_id,
        "https://example.com/tagged",
        "Tagged",
        "example.com",
        Some(vec!["tag".to_string()]),
    );
    bookmark::save(&db.pool, &tagged, "content1").await?;

    let untagged = create_test_bookmark(
        user_id,
        "https://example.com/untagged",
        "Untagged",
        "example.com",
        None,
    );
    bookmark::save(&db.pool, &untagged, "content2").await?;

    // Search with TagFilter::Any (no filtering)
    let search_req = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::Any),
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 2);

    Ok(())
}

#[tokio::test]
async fn test_search_pagination() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create 5 bookmarks
    for i in 1..=5 {
        let bookmark = create_test_bookmark(
            user_id,
            &format!("https://example.com/{}", i),
            &format!("Article {}", i),
            "example.com",
            None,
        );
        bookmark::save(&db.pool, &bookmark, &format!("content {}", i)).await?;
    }

    // Test limit
    let search_req = SearchRequest {
        query: None,
        tags_filter: None,
        limit: Some(3),
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 3);
    assert_eq!(result.total, 5); // Total should still be 5

    // Test offset
    let search_req2 = SearchRequest {
        query: None,
        tags_filter: None,
        limit: Some(3),
        offset: Some(2),
    };

    let result2 = search::search(&db.pool, user_id, &search_req2).await?;

    assert_eq!(result2.items.len(), 3);
    assert_eq!(result2.total, 5);

    // Verify different items are returned
    let first_urls: Vec<&String> = result.items.iter().map(|item| &item.bookmark.url).collect();
    let second_urls: Vec<&String> = result2
        .items
        .iter()
        .map(|item| &item.bookmark.url)
        .collect();

    // There should be some difference due to offset
    let intersection: Vec<&String> = first_urls
        .iter()
        .filter(|url| second_urls.contains(url))
        .copied()
        .collect();
    assert!(
        intersection.len() < 3,
        "Results should differ due to offset"
    );

    Ok(())
}

#[tokio::test]
async fn test_tag_aggregation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with overlapping tags
    let bookmarks_data = vec![
        ("https://example.com/1", vec!["rust", "programming"]),
        ("https://example.com/2", vec!["rust", "web"]),
        ("https://example.com/3", vec!["python", "programming"]),
        ("https://example.com/4", vec!["web", "frontend"]),
    ];

    for (url, tags) in &bookmarks_data {
        let bookmark = create_test_bookmark(
            user_id,
            url,
            "Article",
            "example.com",
            Some(tags.iter().map(|&s| s.to_string()).collect()),
        );
        bookmark::save(&db.pool, &bookmark, "content").await?;
    }

    // Search all bookmarks and check tag aggregation
    let search_req = SearchRequest {
        query: None,
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 4);
    assert_eq!(result.total, 4);

    // Convert to HashMap for easier testing
    let tag_counts: std::collections::HashMap<String, i64> = result
        .tags
        .into_iter()
        .map(|tc| (tc.tag, tc.count))
        .collect();

    assert_eq!(tag_counts.get("rust"), Some(&2));
    assert_eq!(tag_counts.get("programming"), Some(&2));
    assert_eq!(tag_counts.get("web"), Some(&2));
    assert_eq!(tag_counts.get("python"), Some(&1));
    assert_eq!(tag_counts.get("frontend"), Some(&1));

    Ok(())
}

#[tokio::test]
async fn test_combined_search_query_and_tags() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different combinations
    let bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/rust-web",
        "Rust Web Framework",
        "example.com",
        Some(vec!["rust".to_string(), "web".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark1,
        "Learn web development with Rust programming language",
    )
    .await?;

    let bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/rust-cli",
        "Rust CLI Tools",
        "example.com",
        Some(vec!["rust".to_string(), "cli".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark2,
        "Building command line tools with Rust programming",
    )
    .await?;

    let bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/python-web",
        "Python Web Development",
        "example.com",
        Some(vec!["python".to_string(), "web".to_string()]),
    );
    bookmark::save(
        &db.pool,
        &bookmark3,
        "Web development using Python programming language",
    )
    .await?;

    // Search for "programming" text AND "rust" tag
    let search_req = SearchRequest {
        query: Some("programming".to_string()),
        tags_filter: Some(TagFilter::And(vec!["rust".to_string()])),
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 2); // Both rust bookmarks mention programming
    assert_eq!(result.total, 2);

    let urls: Vec<&String> = result.items.iter().map(|item| &item.bookmark.url).collect();
    assert!(urls.contains(&&"https://example.com/rust-web".to_string()));
    assert!(urls.contains(&&"https://example.com/rust-cli".to_string()));
    assert!(!urls.contains(&&"https://example.com/python-web".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_search_ranking() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create bookmarks with different relevance to search term
    let bookmark1 = create_test_bookmark(
        user_id,
        "https://example.com/high-relevance",
        "Rust Programming Language",
        "example.com",
        None,
    );
    bookmark::save(
        &db.pool,
        &bookmark1,
        "Rust is a systems programming language. Rust programming is very popular. Rust Rust Rust.",
    )
    .await?;

    let bookmark2 = create_test_bookmark(
        user_id,
        "https://example.com/medium-relevance",
        "Programming Languages",
        "example.com",
        None,
    );
    bookmark::save(
        &db.pool,
        &bookmark2,
        "Many programming languages exist, including Rust and Python.",
    )
    .await?;

    let bookmark3 = create_test_bookmark(
        user_id,
        "https://example.com/low-relevance",
        "Software Development",
        "example.com",
        None,
    );
    bookmark::save(
        &db.pool,
        &bookmark3,
        "Software development involves many tools and technologies.",
    )
    .await?;

    // Search for "Rust" - should rank by relevance
    let search_req = SearchRequest {
        query: Some("Rust".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    // High relevance should be first
    assert!(result.items.len() >= 2);
    assert!(result.items[0].bookmark.url.contains("high-relevance"));

    // Verify search highlights exist for relevant results
    for item in &result.items {
        if item.bookmark.url.contains("high-relevance")
            || item.bookmark.url.contains("medium-relevance")
        {
            assert!(item.search_match.is_some());
        }
    }

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
        "User1 Article",
        "example.com",
        Some(vec!["shared".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark1, "User1 content about programming").await?;

    let bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/user2",
        "User2 Article",
        "example.com",
        Some(vec!["shared".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark2, "User2 content about programming").await?;

    // Search as user1
    let search_req = SearchRequest {
        query: Some("programming".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result1 = search::search(&db.pool, user1_id, &search_req).await?;

    assert_eq!(result1.items.len(), 1);
    assert_eq!(result1.total, 1);
    assert_eq!(result1.items[0].bookmark.user_id, user1_id);

    // Search as user2
    let result2 = search::search(&db.pool, user2_id, &search_req).await?;

    assert_eq!(result2.items.len(), 1);
    assert_eq!(result2.total, 1);
    assert_eq!(result2.items[0].bookmark.user_id, user2_id);

    // Tag counts should also be isolated
    assert_eq!(result1.tags.len(), 1);
    assert_eq!(result1.tags[0].tag, "shared");
    assert_eq!(result1.tags[0].count, 1); // Only user1's bookmark

    assert_eq!(result2.tags.len(), 1);
    assert_eq!(result2.tags[0].tag, "shared");
    assert_eq!(result2.tags[0].count, 1); // Only user2's bookmark

    Ok(())
}

#[tokio::test]
async fn test_search_empty_results() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create one bookmark
    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/article",
        "Sample Article",
        "example.com",
        Some(vec!["rust".to_string()]),
    );
    bookmark::save(&db.pool, &bookmark, "Content about Rust programming").await?;

    // Search for non-existent term
    let search_req = SearchRequest {
        query: Some("nonexistent".to_string()),
        tags_filter: None,
        limit: None,
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 0);
    assert_eq!(result.total, 0);
    assert_eq!(result.tags.len(), 0);

    // Search for non-existent tag
    let search_req2 = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::And(vec!["nonexistent".to_string()])),
        limit: None,
        offset: None,
    };

    let result2 = search::search(&db.pool, user_id, &search_req2).await?;

    assert_eq!(result2.items.len(), 0);
    assert_eq!(result2.total, 0);
    assert_eq!(result2.tags.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_search_total_count() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create 10 bookmarks
    for i in 1..=10 {
        let bookmark = create_test_bookmark(
            user_id,
            &format!("https://example.com/{}", i),
            &format!("Article {}", i),
            "example.com",
            Some(vec!["tag".to_string()]),
        );
        bookmark::save(&db.pool, &bookmark, &format!("Content for article {}", i)).await?;
    }

    // Search with limit but check total
    let search_req = SearchRequest {
        query: None,
        tags_filter: None,
        limit: Some(3),
        offset: None,
    };

    let result = search::search(&db.pool, user_id, &search_req).await?;

    assert_eq!(result.items.len(), 3); // Limited results
    assert_eq!(result.total, 10); // Total count should be accurate

    // Search with tag filter
    let search_req2 = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::And(vec!["tag".to_string()])),
        limit: Some(5),
        offset: None,
    };

    let result2 = search::search(&db.pool, user_id, &search_req2).await?;

    assert_eq!(result2.items.len(), 5);
    assert_eq!(result2.total, 10); // All have the "tag"

    Ok(())
}

#[tokio::test]
async fn test_concurrent_search() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = create_test_user(&db).await?;

    // Create test data
    for i in 1..=5 {
        let bookmark = create_test_bookmark(
            user_id,
            &format!("https://example.com/{}", i),
            &format!("Article {}", i),
            "example.com",
            Some(vec!["concurrent".to_string()]),
        );
        bookmark::save(&db.pool, &bookmark, &format!("Content {}", i)).await?;
    }

    let search_req = SearchRequest {
        query: None,
        tags_filter: Some(TagFilter::And(vec!["concurrent".to_string()])),
        limit: None,
        offset: None,
    };

    // Run concurrent searches
    let pool1 = db.pool.clone();
    let pool2 = db.pool.clone();
    let req1 = search_req.clone();
    let req2 = search_req.clone();

    let handle1 = tokio::spawn(async move { search::search(&pool1, user_id, &req1).await });

    let handle2 = tokio::spawn(async move { search::search(&pool2, user_id, &req2).await });

    let result1 = handle1.await??;
    let result2 = handle2.await??;

    // Both should return same results
    assert_eq!(result1.items.len(), result2.items.len());
    assert_eq!(result1.total, result2.total);
    assert_eq!(result1.tags.len(), result2.tags.len());

    // Results should be consistent
    assert_eq!(result1.items.len(), 5);
    assert_eq!(result1.total, 5);

    Ok(())
}
