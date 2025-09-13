#![cfg(feature = "integration-tests")]

mod common;

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
