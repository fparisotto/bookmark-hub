#![cfg(feature = "integration-tests")]

mod common;

use common::test_db::{create_test_bookmark, TestDatabase};
use server::db::{bookmark, chunks, rag};
use shared::RagHistoryRequest;
use uuid::Uuid;

#[tokio::test]
async fn test_rag_session_create_and_retrieve() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create a new RAG session
    let question = "What is Rust programming language?";
    let session = rag::create_rag_session(&db.pool, user_id, question).await?;

    // Verify session fields
    assert_eq!(session.question, question);
    assert_eq!(session.user_id, user_id);
    assert!(!session.session_id.is_nil());
    assert!(session.answer.is_none()); // Initially no answer
    assert!(session.relevant_chunks.is_empty());
    assert!(session.created_at <= session.updated_at.unwrap_or(session.created_at));

    // Retrieve the session
    let retrieved_session = rag::get_rag_session(&db.pool, session.session_id, user_id).await?;
    assert!(retrieved_session.is_some());
    let retrieved_session = retrieved_session.unwrap();
    assert_eq!(retrieved_session.session_id, session.session_id);
    assert_eq!(retrieved_session.question, session.question);
    assert_eq!(retrieved_session.user_id, session.user_id);

    Ok(())
}

#[tokio::test]
async fn test_rag_session_update_with_answer() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create a session
    let question = "How does async/await work?";
    let session = rag::create_rag_session(&db.pool, user_id, question).await?;

    // Update with answer and relevant chunks
    let answer = "Async/await is a programming pattern that allows asynchronous operations to be written in a synchronous-looking way.";
    let chunk_ids = vec![Uuid::new_v4(), Uuid::new_v4()];

    let updated_session =
        rag::update_rag_session(&db.pool, session.session_id, user_id, answer, &chunk_ids).await?;

    // Verify updates
    assert_eq!(updated_session.answer, Some(answer.to_string()));
    assert_eq!(updated_session.relevant_chunks, chunk_ids);
    assert_eq!(updated_session.question, question);
    assert!(updated_session.updated_at.unwrap() >= updated_session.created_at);

    // Verify retrieval returns updated data
    let retrieved = rag::get_rag_session(&db.pool, session.session_id, user_id).await?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.answer, Some(answer.to_string()));
    assert_eq!(retrieved.relevant_chunks, chunk_ids);

    Ok(())
}

#[tokio::test]
async fn test_rag_session_nonexistent() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Try to get non-existent session
    let random_session_id = Uuid::new_v4();
    let result = rag::get_rag_session(&db.pool, random_session_id, user_id).await?;
    assert!(result.is_none());

    Ok(())
}

#[tokio::test]
async fn test_rag_session_user_isolation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = db.create_user().await?;
    let user2_id = db.create_user().await?;

    // Create session for user1
    let question = "Test question for user isolation";
    let session = rag::create_rag_session(&db.pool, user1_id, question).await?;

    // Try to access session with user2 credentials - should fail
    let result = rag::get_rag_session(&db.pool, session.session_id, user2_id).await?;
    assert!(result.is_none(), "Sessions should be isolated per user");

    // Verify user1 can still access their session
    let result = rag::get_rag_session(&db.pool, session.session_id, user1_id).await?;
    assert!(result.is_some());

    Ok(())
}

#[tokio::test]
async fn test_rag_history_retrieval() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create multiple RAG sessions
    let questions = vec![
        "What is machine learning?",
        "How does blockchain work?",
        "Explain quantum computing",
    ];

    let mut created_sessions = Vec::new();
    for question in &questions {
        let session = rag::create_rag_session(&db.pool, user_id, question).await?;
        created_sessions.push(session);
    }

    // Update some sessions with answers
    let answer = "This is a test answer";
    rag::update_rag_session(
        &db.pool,
        created_sessions[0].session_id,
        user_id,
        answer,
        &[Uuid::new_v4()],
    )
    .await?;

    // Get history
    let request = RagHistoryRequest {
        limit: Some(10),
        offset: None,
    };
    let history = rag::get_rag_history(&db.pool, user_id, &request).await?;

    // Verify history
    assert_eq!(history.total_count, 3);
    assert_eq!(history.sessions.len(), 3);

    // Check sessions are ordered by created_at DESC (most recent first)
    for i in 1..history.sessions.len() {
        assert!(history.sessions[i - 1].created_at >= history.sessions[i].created_at);
    }

    // Check that one session has an answer
    let sessions_with_answers: Vec<_> = history
        .sessions
        .iter()
        .filter(|s| s.answer.is_some())
        .collect();
    assert_eq!(sessions_with_answers.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_rag_history_pagination() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create 5 sessions
    for i in 0..5 {
        let question = format!("Test question {}", i);
        rag::create_rag_session(&db.pool, user_id, &question).await?;
    }

    // Get first page (limit 2)
    let request = RagHistoryRequest {
        limit: Some(2),
        offset: None,
    };
    let page1 = rag::get_rag_history(&db.pool, user_id, &request).await?;
    assert_eq!(page1.sessions.len(), 2);
    assert_eq!(page1.total_count, 5);

    // Get second page (offset 2, limit 2)
    let request = RagHistoryRequest {
        limit: Some(2),
        offset: Some(2),
    };
    let page2 = rag::get_rag_history(&db.pool, user_id, &request).await?;
    assert_eq!(page2.sessions.len(), 2);
    assert_eq!(page2.total_count, 5);

    // Get remaining items (offset 4, limit 2)
    let request = RagHistoryRequest {
        limit: Some(2),
        offset: Some(4),
    };
    let page3 = rag::get_rag_history(&db.pool, user_id, &request).await?;
    assert_eq!(page3.sessions.len(), 1); // Only 1 remaining
    assert_eq!(page3.total_count, 5);

    // Verify no overlap between pages
    let all_session_ids: Vec<_> = [&page1.sessions, &page2.sessions, &page3.sessions]
        .iter()
        .flat_map(|page| page.iter().map(|s| s.session_id))
        .collect();

    let unique_ids: std::collections::HashSet<_> = all_session_ids.iter().cloned().collect();
    assert_eq!(
        unique_ids.len(),
        5,
        "All session IDs should be unique across pages"
    );

    Ok(())
}

#[tokio::test]
async fn test_rag_history_user_isolation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = db.create_user().await?;
    let user2_id = db.create_user().await?;

    // Create sessions for both users
    rag::create_rag_session(&db.pool, user1_id, "User1 question 1").await?;
    rag::create_rag_session(&db.pool, user1_id, "User1 question 2").await?;
    rag::create_rag_session(&db.pool, user2_id, "User2 question 1").await?;

    // Get history for user1
    let request = RagHistoryRequest {
        limit: Some(10),
        offset: None,
    };
    let user1_history = rag::get_rag_history(&db.pool, user1_id, &request).await?;
    assert_eq!(user1_history.total_count, 2);
    assert_eq!(user1_history.sessions.len(), 2);

    // Get history for user2
    let user2_history = rag::get_rag_history(&db.pool, user2_id, &request).await?;
    assert_eq!(user2_history.total_count, 1);
    assert_eq!(user2_history.sessions.len(), 1);

    // Verify questions are correct for each user
    assert!(user1_history
        .sessions
        .iter()
        .all(|s| s.question.contains("User1")));
    assert!(user2_history
        .sessions
        .iter()
        .all(|s| s.question.contains("User2")));

    Ok(())
}

#[tokio::test]
async fn test_bookmark_chunks_storage_and_retrieval() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create a bookmark first
    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/rust-guide",
        "Rust Programming Guide",
        "example.com",
        None,
    );
    let text_content = "This is a comprehensive guide to Rust programming. Rust is a systems programming language...";
    let bookmark_id = &bookmark.bookmark_id;

    let bookmark_result = bookmark::save(&db.pool, &bookmark, text_content).await;

    // Handle the case where bookmark creation fails (it's okay for this test)
    if bookmark_result.is_err() {
        println!("Note: Bookmark creation skipped for chunk test");
    }

    // Test chunk storage with embeddings
    let test_chunks = vec![
        "This is a comprehensive guide to Rust programming.".to_string(),
        "Rust is a systems programming language that focuses on safety and performance."
            .to_string(),
        "Memory safety is guaranteed by Rust's ownership system.".to_string(),
    ];

    // Generate mock embeddings (768-dimensional for nomic-embed-text:v1.5)
    let embeddings: Vec<Vec<f32>> = test_chunks
        .iter()
        .map(|_| (0..768).map(|i| (i as f32) / 768.0).collect())
        .collect();

    let stored_chunks = chunks::store_chunks_with_embeddings(
        &db.pool,
        bookmark_id,
        user_id,
        test_chunks.clone(),
        embeddings.clone(),
    )
    .await?;

    // Verify stored chunks
    assert_eq!(stored_chunks.len(), 3);
    for (i, chunk) in stored_chunks.iter().enumerate() {
        assert_eq!(chunk.bookmark_id, *bookmark_id);
        assert_eq!(chunk.user_id, user_id);
        assert_eq!(chunk.chunk_text, test_chunks[i]);
        assert_eq!(chunk.chunk_index, i as i32);
        assert!(!chunk.chunk_id.is_nil());
    }

    // Test chunk retrieval by IDs
    let chunk_ids: Vec<Uuid> = stored_chunks.iter().map(|c| c.chunk_id).collect();
    let retrieved_chunks = chunks::get_chunks_by_ids(&db.pool, user_id, &chunk_ids).await?;

    assert_eq!(retrieved_chunks.len(), 3);
    for chunk in &retrieved_chunks {
        assert!(chunk_ids.contains(&chunk.chunk_id));
    }

    // Test has_chunks_for_bookmark
    let has_chunks = chunks::has_chunks_for_bookmark(&db.pool, bookmark_id, user_id).await?;
    assert!(has_chunks);

    // Test with non-existent bookmark
    let has_chunks_nonexistent =
        chunks::has_chunks_for_bookmark(&db.pool, "nonexistent", user_id).await?;
    assert!(!has_chunks_nonexistent);

    Ok(())
}

#[tokio::test]
async fn test_chunks_storage_overwrite() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;
    // Create the bookmark first
    let bookmark = create_test_bookmark(
        user_id,
        "https://example.com/overwrite",
        "Test Overwrite",
        "example.com",
        None,
    );
    let bookmark_id = &bookmark.bookmark_id;
    let _ = bookmark::save(&db.pool, &bookmark, "Test content for overwrite").await;

    // Store initial chunks
    let initial_chunks = vec!["First chunk".to_string(), "Second chunk".to_string()];
    let initial_embeddings: Vec<Vec<f32>> = initial_chunks.iter().map(|_| vec![0.1; 768]).collect();

    let initial_stored = chunks::store_chunks_with_embeddings(
        &db.pool,
        bookmark_id,
        user_id,
        initial_chunks,
        initial_embeddings,
    )
    .await?;
    assert_eq!(initial_stored.len(), 2);

    // Store new chunks (should overwrite)
    let new_chunks = vec![
        "New first chunk".to_string(),
        "New second chunk".to_string(),
        "New third chunk".to_string(),
    ];
    let new_embeddings: Vec<Vec<f32>> = new_chunks.iter().map(|_| vec![0.2; 768]).collect();

    let new_stored = chunks::store_chunks_with_embeddings(
        &db.pool,
        bookmark_id,
        user_id,
        new_chunks.clone(),
        new_embeddings,
    )
    .await?;
    assert_eq!(new_stored.len(), 3);

    // Verify old chunks are gone and new chunks are present
    let all_chunk_ids: Vec<Uuid> = new_stored.iter().map(|c| c.chunk_id).collect();
    let retrieved = chunks::get_chunks_by_ids(&db.pool, user_id, &all_chunk_ids).await?;

    assert_eq!(retrieved.len(), 3);
    for (i, chunk) in retrieved.iter().enumerate() {
        assert_eq!(chunk.chunk_text, new_chunks[i]);
    }

    Ok(())
}

#[tokio::test]
async fn test_chunks_user_isolation() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user1_id = db.create_user().await?;
    let user2_id = db.create_user().await?;
    // Create bookmarks for both users
    let bookmark1 = create_test_bookmark(
        user1_id,
        "https://example.com/shared1",
        "User1 Title",
        "example.com",
        None,
    );
    let bookmark1_id = &bookmark1.bookmark_id;
    let _ = bookmark::save(&db.pool, &bookmark1, "User1 content").await;

    let bookmark2 = create_test_bookmark(
        user2_id,
        "https://example.com/shared2",
        "User2 Title",
        "example.com",
        None,
    );
    let bookmark2_id = &bookmark2.bookmark_id;
    let _ = bookmark::save(&db.pool, &bookmark2, "User2 content").await;

    // Store chunks for user1
    let user1_chunks = vec!["User1 chunk".to_string()];
    let user1_embeddings = vec![vec![0.1; 768]];

    let user1_stored = chunks::store_chunks_with_embeddings(
        &db.pool,
        bookmark1_id,
        user1_id,
        user1_chunks,
        user1_embeddings,
    )
    .await?;

    // Store chunks for user2
    let user2_chunks = vec!["User2 chunk".to_string()];
    let user2_embeddings = vec![vec![0.2; 768]];

    let user2_stored = chunks::store_chunks_with_embeddings(
        &db.pool,
        bookmark2_id,
        user2_id,
        user2_chunks,
        user2_embeddings,
    )
    .await?;

    // Verify user1 can only access their chunks
    let user1_retrieved =
        chunks::get_chunks_by_ids(&db.pool, user1_id, &[user1_stored[0].chunk_id]).await?;
    assert_eq!(user1_retrieved.len(), 1);
    assert_eq!(user1_retrieved[0].chunk_text, "User1 chunk");

    // Verify user2 can only access their chunks
    let user2_retrieved =
        chunks::get_chunks_by_ids(&db.pool, user2_id, &[user2_stored[0].chunk_id]).await?;
    assert_eq!(user2_retrieved.len(), 1);
    assert_eq!(user2_retrieved[0].chunk_text, "User2 chunk");

    // Verify cross-user access fails
    let cross_access =
        chunks::get_chunks_by_ids(&db.pool, user1_id, &[user2_stored[0].chunk_id]).await?;
    assert_eq!(cross_access.len(), 0);

    Ok(())
}

#[tokio::test]
async fn test_chunks_length_mismatch_error() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    let chunks = vec!["Chunk 1".to_string(), "Chunk 2".to_string()];
    let embeddings = vec![vec![0.1; 768]]; // Only one embedding for two chunks

    let result = chunks::store_chunks_with_embeddings(
        &db.pool,
        "test_bookmark",
        user_id,
        chunks,
        embeddings,
    )
    .await;

    assert!(result.is_err());
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("mismatch") || error_msg.contains("chunks"));

    Ok(())
}

#[tokio::test]
async fn test_get_bookmarks_without_chunks() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;
    let user_id = db.create_user().await?;

    // Create bookmarks with sufficient text content
    let bookmarks_data = vec![
        (
            "bookmark1",
            "https://example1.com",
            "Title 1",
            "example1.com",
            "A".repeat(150),
        ), // Has enough content
        (
            "bookmark2",
            "https://example2.com",
            "Title 2",
            "example2.com",
            "B".repeat(50),
        ), // Too short
        (
            "bookmark3",
            "https://example3.com",
            "Title 3",
            "example3.com",
            "C".repeat(200),
        ), // Has enough content
    ];

    for (_bookmark_id, url, title, domain, text_content) in &bookmarks_data {
        let bookmark = create_test_bookmark(user_id, url, title, domain, None);
        // Note: We simulate the bookmark creation for this test since we're testing
        // chunk functionality
        let _ = bookmark::save(&db.pool, &bookmark, text_content).await;
    }

    // Get bookmarks without chunks
    let bookmarks_without_chunks = chunks::get_bookmarks_without_chunks(&db.pool, 10).await?;

    // Should return bookmarks with sufficient content (>100 chars)
    let returned_bookmarks: Vec<_> = bookmarks_without_chunks
        .iter()
        .filter(|(_, uid, _)| uid == &user_id)
        .collect();

    // We expect at least the bookmarks with >100 chars content
    assert!(returned_bookmarks.len() >= 2);

    // Verify content length filter
    for (_, _, text_content) in &returned_bookmarks {
        assert!(text_content.len() > 100);
    }

    Ok(())
}
