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

    let canonical_url_column_exists: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1
                FROM information_schema.columns
                WHERE table_schema = 'public'
                  AND table_name = 'bookmark'
                  AND column_name = 'canonical_url'
            )",
            &[],
        )
        .await
        .context("Failed to query canonical_url column metadata")?
        .get(0);
    assert!(
        canonical_url_column_exists,
        "bookmark.canonical_url should exist after migrations"
    );
    let canonical_url_constraint_exists: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'bookmark_canonical_url_user_unique'
            )",
            &[],
        )
        .await
        .context("Failed to query bookmark canonical URL uniqueness constraint")?
        .get(0);
    assert!(
        canonical_url_constraint_exists,
        "bookmark canonical URL uniqueness constraint should exist after migrations"
    );

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
            "INSERT INTO bookmark (bookmark_id, user_id, url, canonical_url, domain, title, text_content, tags) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            &[
                &"bookmark-migration-4",
                &user_id,
                &"https://example.com",
                &"https://example.com/",
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
    assert_eq!(schema_version, 7);

    Ok(())
}

#[tokio::test]
async fn test_db_migration_7_backfills_bookmark_identity() -> anyhow::Result<()> {
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
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/4_trim_tags.sql"
        )))
        .await
        .context("Failed to apply schema version 4")?;
    client
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/5_reembed_qwen3.sql"
        )))
        .await
        .context("Failed to apply schema version 5")?;
    client
        .batch_execute(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/6_ai_generation_retry_state.sql"
        )))
        .await
        .context("Failed to apply schema version 6")?;

    client
        .batch_execute(
            r#"
            ALTER TABLE bookmark DROP CONSTRAINT IF EXISTS bookmark_canonical_url_user_unique;
            ALTER TABLE bookmark DROP COLUMN canonical_url;
            ALTER TABLE bookmark ADD CONSTRAINT bookmark_url_user_unique UNIQUE (url, user_id);
            ALTER TABLE bookmark ADD CONSTRAINT bookmark_bookmark_id_key UNIQUE (bookmark_id);
            "#,
        )
        .await
        .context("Failed to reshape bookmark table to pre-migration-7 state")?;

    client
        .execute(
            "INSERT INTO \"user\" (user_id, username, password_hash) VALUES ($1, $2, $3)",
            &[
                &uuid::Uuid::new_v4(),
                &"migration-test-user-7",
                &"password_hash",
            ],
        )
        .await
        .context("Failed to insert test user")?;

    let user_id: uuid::Uuid = client
        .query_one(
            "SELECT user_id FROM \"user\" WHERE username = $1",
            &[&"migration-test-user-7"],
        )
        .await
        .context("Failed to fetch test user")?
        .get(0);

    client
        .execute(
            "INSERT INTO bookmark (bookmark_id, user_id, url, domain, title, text_content) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &"bookmark-migration-7",
                &user_id,
                &"https://EXAMPLE.com:443/post?a=1#section",
                &"example.com",
                &"Example",
                &"Example body",
            ],
        )
        .await
        .context("Failed to insert bookmark before migration 7")?;

    drop(client);

    db::run_migrations(&db.pool)
        .await
        .context("Failed to apply pending migrations after schema version 6")?;

    let client = db.pool.get().await?;
    let canonical_url: String = client
        .query_one(
            "SELECT canonical_url FROM bookmark WHERE bookmark_id = $1 AND user_id = $2",
            &[&"bookmark-migration-7", &user_id],
        )
        .await
        .context("Failed to query canonical_url after migration 7")?
        .get(0);
    let has_per_user_canonical_unique: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'bookmark_canonical_url_user_unique'
            )",
            &[],
        )
        .await
        .context("Failed to query per-user canonical URL constraint")?
        .get(0);
    let has_global_bookmark_id_unique: bool = client
        .query_one(
            "SELECT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'bookmark_bookmark_id_key'
            )",
            &[],
        )
        .await
        .context("Failed to query legacy bookmark_id uniqueness constraint")?
        .get(0);

    assert_eq!(canonical_url, "https://example.com/post?a=1");
    assert!(has_per_user_canonical_unique);
    assert!(!has_global_bookmark_id_unique);

    Ok(())
}
