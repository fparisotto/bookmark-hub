#![cfg(feature = "integration-tests")]

mod common;

use common::test_db::TestDatabase;
use server::db::user;
use uuid::Uuid;

#[tokio::test]
async fn test_user_create_and_retrieve() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Create a new user
    let username = "testuser".to_string();
    let password_hash = "hashed_password_123".to_string();

    let created_user = user::create(&db.pool, username.clone(), password_hash.clone()).await?;

    // Verify user fields
    assert_eq!(created_user.username, username);
    assert_eq!(created_user.password_hash, password_hash);
    assert!(!created_user.user_id.is_nil());
    assert!(created_user.created_at <= created_user.updated_at);

    // Test get_by_id
    let user_by_id = user::get_by_id(&db.pool, &created_user.user_id).await?;
    assert!(user_by_id.is_some());
    let user_by_id = user_by_id.unwrap();
    assert_eq!(user_by_id.user_id, created_user.user_id);
    assert_eq!(user_by_id.username, created_user.username);
    assert_eq!(user_by_id.password_hash, created_user.password_hash);

    // Test get_by_username
    let user_by_username = user::get_by_username(&db.pool, username).await?;
    assert!(user_by_username.is_some());
    let user_by_username = user_by_username.unwrap();
    assert_eq!(user_by_username.user_id, created_user.user_id);
    assert_eq!(user_by_username.username, created_user.username);
    assert_eq!(user_by_username.password_hash, created_user.password_hash);

    Ok(())
}

#[tokio::test]
async fn test_duplicate_username_constraint() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    let username = "duplicateuser".to_string();
    let password_hash1 = "hash1".to_string();
    let password_hash2 = "hash2".to_string();

    // Create first user - should succeed
    let first_user = user::create(&db.pool, username.clone(), password_hash1).await?;
    assert_eq!(first_user.username, username);

    // Try to create second user with same username - should fail
    let result = user::create(&db.pool, username, password_hash2).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_message = format!("{:?}", error);

    // Verify the error is about unique username constraint
    assert!(
        error_message.contains("unique_username")
            || error_message.contains("username already used")
    );

    Ok(())
}

#[tokio::test]
async fn test_case_insensitive_username_uniqueness() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Create user with mixed case username
    let username1 = "TestUser".to_string();
    let password_hash1 = "hash1".to_string();

    let first_user = user::create(&db.pool, username1.clone(), password_hash1).await?;
    assert_eq!(first_user.username, username1);

    // Try to create user with same username but different case - should fail
    let username2 = "testuser".to_string(); // lowercase version
    let password_hash2 = "hash2".to_string();

    let result = user::create(&db.pool, username2, password_hash2).await;

    assert!(result.is_err());
    // The constraint should trigger due to LOWER(username) unique index

    // Also test with uppercase
    let username3 = "TESTUSER".to_string();
    let result2 = user::create(&db.pool, username3, "hash3".to_string()).await;
    assert!(result2.is_err());

    Ok(())
}

#[tokio::test]
async fn test_get_by_id_nonexistent() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Generate a random UUID that doesn't exist
    let random_id = Uuid::new_v4();

    let result = user::get_by_id(&db.pool, &random_id).await?;
    assert!(result.is_none());

    Ok(())
}

#[tokio::test]
async fn test_get_by_username_nonexistent() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    let nonexistent_username = "this_user_does_not_exist".to_string();

    let result = user::get_by_username(&db.pool, nonexistent_username).await?;
    assert!(result.is_none());

    Ok(())
}

#[tokio::test]
async fn test_multiple_users() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Create multiple users
    let users_data = vec![
        ("alice", "alice_hash"),
        ("bob", "bob_hash"),
        ("charlie", "charlie_hash"),
    ];

    let mut created_users = Vec::new();

    for (username, password_hash) in &users_data {
        let user = user::create(&db.pool, username.to_string(), password_hash.to_string()).await?;
        created_users.push(user);
    }

    // Verify all users were created with different IDs
    assert_eq!(created_users.len(), 3);
    let user_ids: Vec<Uuid> = created_users.iter().map(|u| u.user_id).collect();
    let unique_ids: std::collections::HashSet<Uuid> = user_ids.iter().cloned().collect();
    assert_eq!(unique_ids.len(), 3, "All users should have unique IDs");

    // Verify each user can be retrieved independently
    for (i, user) in created_users.iter().enumerate() {
        let (expected_username, expected_hash) = users_data[i];

        // Get by ID
        let retrieved = user::get_by_id(&db.pool, &user.user_id).await?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.username, expected_username);
        assert_eq!(retrieved.password_hash, expected_hash);

        // Get by username
        let retrieved = user::get_by_username(&db.pool, expected_username.to_string()).await?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.user_id, user.user_id);
        assert_eq!(retrieved.password_hash, expected_hash);
    }

    Ok(())
}

#[tokio::test]
async fn test_username_with_special_characters() -> anyhow::Result<()> {
    let db = TestDatabase::new().await?;

    // Test usernames with various special characters
    let special_usernames = vec![
        "user.name",
        "user-name",
        "user_name",
        "user123",
        "user@example",
        "user name", // with space
    ];

    for username in special_usernames {
        let password_hash = format!("hash_for_{}", username);

        let created_user =
            user::create(&db.pool, username.to_string(), password_hash.clone()).await?;
        assert_eq!(created_user.username, username);

        // Verify retrieval works
        let retrieved = user::get_by_username(&db.pool, username.to_string()).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username, username);
    }

    Ok(())
}
