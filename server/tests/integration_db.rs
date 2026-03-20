#![cfg(feature = "integration-tests")]

mod common;

use std::collections::BTreeSet;

use anyhow::Context;
use common::test_db::TestDatabase;
use server::db;

#[tokio::test]
async fn test_db_database_migrations() -> anyhow::Result<()> {
    // Create an empty database
    let db = TestDatabase::new_empty().await?;
    let client = db.pool.get().await?;

    // Run migrations
    db::run_migrations(&db.pool)
        .await
        .context("Failed to run database migrations on the newly created database")?;

    // Verify that essential tables exist
    let tables = vec!["user", "bookmark", "bookmark_task", "schema_version"];

    for table in tables {
        let sql = format!(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'public'
                AND table_name = '{table}'
            );"
        );
        let exists_check: bool = client
            .query_one(&sql, &[])
            .await
            .context(format!("Failed to query for table {table}"))?
            .get(0);
        assert!(exists_check, "Table {table} should exist after migrations");
    }

    // Verify schema_version table is populated
    let schema_version: i32 = client
        .query_one("SELECT MAX(version) FROM schema_version", &[])
        .await
        .context("Failed to query schema_version table")?
        .get(0);

    assert!(
        schema_version > 0,
        "Schema version should be greater than 0"
    );
    println!("Migrations successfully applied. Current schema version: {schema_version}");

    Ok(())
}

#[tokio::test]
async fn test_db_with_migrations_preapplied() -> anyhow::Result<()> {
    // This test creates a database with migrations already applied
    let db = TestDatabase::new().await?;

    // Verify we can connect and query
    let client = db.pool.get().await?;
    let result: i32 = client.query_one("SELECT 1", &[]).await?.get(0);

    assert_eq!(result, 1, "Basic query should work");

    // Verify we can check the schema version
    let schema_version: i32 = client
        .query_one("SELECT MAX(version) FROM schema_version", &[])
        .await?
        .get(0);

    assert!(schema_version > 0, "Schema version should be set");
    println!("Test database created successfully with schema version: {schema_version}");

    Ok(())
}

#[tokio::test]
async fn test_db_migration_4_backfills_existing_version_3_data() -> anyhow::Result<()> {
    let db = TestDatabase::new_empty().await?;
    let client = db.pool.get().await?;

    client
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/1_unified.sql"
        )))
        .await
        .context("Failed to apply schema version 1")?;
    client
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/2_embedding_dimension.sql"
        )))
        .await
        .context("Failed to apply schema version 2")?;
    client
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/3_lowercase_tags.sql"
        )))
        .await
        .context("Failed to apply schema version 3")?;

    client
        .execute(
            "INSERT INTO \"user\" (user_id, username, password_hash) VALUES ($1, $2, $3)",
            &[
                &uuid::Uuid::new_v4(),
                &"migration-test-user",
                &"password_hash",
            ],
        )
        .await
        .context("Failed to insert test user")?;

    let user_id: uuid::Uuid = client
        .query_one(
            "SELECT user_id FROM \"user\" WHERE username = $1",
            &[&"migration-test-user"],
        )
        .await
        .context("Failed to fetch test user")?
        .get(0);

    client
        .execute(
            "INSERT INTO bookmark (bookmark_id, user_id, url, domain, title, text_content, tags) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            &[
                &"bookmark-migration-4",
                &user_id,
                &"https://example.com",
                &"example.com",
                &"Example",
                &"Example body",
                &vec![
                    " Rust ".to_string(),
                    "rust".to_string(),
                    "   ".to_string(),
                    "".to_string(),
                    " SQL ".to_string(),
                ],
            ],
        )
        .await
        .context("Failed to insert bookmark with dirty tags")?;

    client
        .execute(
            "INSERT INTO bookmark_task (user_id, url, tags) VALUES ($1, $2, $3)",
            &[
                &user_id,
                &"https://example.com/task",
                &vec![
                    " Ops ".to_string(),
                    "ops".to_string(),
                    " ".to_string(),
                    "".to_string(),
                ],
            ],
        )
        .await
        .context("Failed to insert bookmark task with dirty tags")?;

    drop(client);

    db::run_migrations(&db.pool)
        .await
        .context("Failed to apply pending migrations after schema version 3")?;

    let client = db.pool.get().await?;
    let bookmark_tags: Vec<String> = client
        .query_one(
            "SELECT tags FROM bookmark WHERE bookmark_id = $1 AND user_id = $2",
            &[&"bookmark-migration-4", &user_id],
        )
        .await
        .context("Failed to query normalized bookmark tags")?
        .get(0);
    let task_tags: Vec<String> = client
        .query_one(
            "SELECT tags FROM bookmark_task WHERE user_id = $1 AND url = $2",
            &[&user_id, &"https://example.com/task"],
        )
        .await
        .context("Failed to query normalized bookmark task tags")?
        .get(0);
    let schema_version: i32 = client
        .query_one("SELECT MAX(version) FROM schema_version", &[])
        .await
        .context("Failed to query schema version after backfill migration")?
        .get(0);

    assert_eq!(
        bookmark_tags.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from(["rust".to_string(), "sql".to_string()])
    );
    assert_eq!(
        task_tags.into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from(["ops".to_string()])
    );
    assert_eq!(schema_version, 4);

    Ok(())
}
