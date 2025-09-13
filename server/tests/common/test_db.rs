#![allow(dead_code)]

use anyhow::Context;
use server::db::{self, user};
use shared::Bookmark;
use testcontainers::{ContainerAsync, GenericImage};
use uuid::Uuid;

use super::{create_postgres_container, create_postgres_pool, generate_test_db_name, PgPool};

/// The Docker image to use for the PostgreSQL container.
/// Can be overridden by the `TEST_DB_CONTAINER_IMAGE` environment variable.
const CONTAINER_IMAGE: &str = match option_env!("TEST_DB_CONTAINER_IMAGE") {
    Some(image) => image,
    None => "postgres:17-alpine",
};

/// An isolated database instance for a single test.
///
/// This struct provides an isolated PostgreSQL database for testing,
/// automatically handling database creation before a test and cleanup
/// afterward.
pub struct TestDatabase {
    pub pool: PgPool,
    pub db_name: String,

    // Container reference to keep it alive
    _container: ContainerAsync<GenericImage>,

    // Connection details
    pub host: String,
    pub port: u16,
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Close the connection pool to prevent any lingering connections
        self.pool.close();
        // Container cleanup is handled automatically when _container is dropped
    }
}

impl TestDatabase {
    /// Creates a new, randomly named test database with migrations applied.
    pub async fn new() -> anyhow::Result<Self> {
        let test_id = Uuid::new_v4().simple().to_string();
        let container_name = format!("test_db_{test_id}");

        println!("Starting PostgreSQL test container with ID: {test_id}");

        let container = start_postgres_container(&container_name).await;
        let host = container
            .get_host()
            .await
            .context("Container should have accessible host address")?
            .to_string();
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .context("Container should expose PostgreSQL port 5432")?;

        println!("PostgreSQL container ready at: {host}:{port}");

        let db_name = generate_test_db_name("test");

        // Create the database using the default 'postgres' database
        let admin_pool = create_postgres_pool(&host, port, "postgres", "postgres", None).await?;
        let client = admin_pool.get().await?;
        client
            .execute(
                &format!("CREATE DATABASE \"{db_name}\" WITH ENCODING 'UTF8'"),
                &[],
            )
            .await?;

        // Connect to the new database
        let pool = create_postgres_pool(&host, port, &db_name, "postgres", None).await?;

        // Run migrations
        db::run_migrations(&pool)
            .await
            .context("Failed to run migrations")?;

        Ok(Self {
            pool,
            db_name,
            _container: container,
            host,
            port,
        })
    }

    /// Creates a new, randomly named, and completely empty test database.
    ///
    /// This function does not run migrations. It is intended for
    /// special cases like testing initial database migrations from scratch.
    pub async fn new_empty() -> anyhow::Result<Self> {
        let test_id = Uuid::new_v4().simple().to_string();
        let container_name = format!("test_db_empty_{test_id}");

        println!("Starting empty PostgreSQL test container with ID: {test_id}");

        let container = start_postgres_container(&container_name).await;
        let host = container
            .get_host()
            .await
            .context("Container should have accessible host address")?
            .to_string();
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .context("Container should expose PostgreSQL port 5432")?;

        let db_name = generate_test_db_name("db_test");

        let admin_pool = create_postgres_pool(&host, port, "postgres", "postgres", None).await?;
        let client = admin_pool.get().await?;
        client
            .execute(
                &format!("CREATE DATABASE \"{db_name}\" WITH ENCODING 'UTF8'"),
                &[],
            )
            .await?;

        let pool = create_postgres_pool(&host, port, &db_name, "postgres", None).await?;

        Ok(Self {
            pool,
            db_name,
            _container: container,
            host,
            port,
        })
    }

    /// Creates a test user and returns the user ID.
    pub async fn create_user(&self) -> anyhow::Result<Uuid> {
        create_test_user(self).await
    }
}

/// Creates a test user with a unique username.
pub async fn create_test_user(db: &TestDatabase) -> anyhow::Result<Uuid> {
    let username = format!("test_user_{}", Uuid::new_v4());
    let user = user::create(&db.pool, username, "password_hash".to_string()).await?;
    Ok(user.user_id)
}

/// Creates a test bookmark with the specified parameters.
/// This is the enhanced version that supports optional tags and covers all use
/// cases.
pub fn create_test_bookmark(
    user_id: Uuid,
    url: &str,
    title: &str,
    domain: &str,
    tags: Option<Vec<String>>,
) -> Bookmark {
    Bookmark {
        bookmark_id: format!("bookmark_{}", Uuid::new_v4()),
        user_id,
        url: url.to_string(),
        domain: domain.to_string(),
        title: title.to_string(),
        tags,
        summary: None,
        created_at: chrono::Utc::now(),
        updated_at: None,
    }
}

// ==== Container Management =====

/// Starts the PostgreSQL container with optimized settings for testing.
async fn start_postgres_container(container_name: &str) -> ContainerAsync<GenericImage> {
    create_postgres_container(
        CONTAINER_IMAGE,
        container_name,
        "postgres",
        None,
        "postgres",
    )
    .await
    .expect("PostgreSQL container should start successfully")
}
