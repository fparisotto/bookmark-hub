#![allow(dead_code)]

use anyhow::Context;
use deadpool_postgres::{Config, ManagerConfig, PoolConfig, RecyclingMethod, Runtime};
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use uuid::Uuid;

pub mod test_db;

pub type PgPool = deadpool_postgres::Pool;

/// Generates a unique database name with a random suffix.
pub(crate) fn generate_test_db_name(prefix: &str) -> String {
    let rng_suffix = Uuid::new_v4().simple();
    format!("{prefix}_{rng_suffix}")
}

/// Creates a PostgreSQL connection pool with common test settings.
pub(crate) async fn create_postgres_pool(
    host: &str,
    port: u16,
    db_name: &str,
    user: &str,
    password: Option<&str>,
) -> anyhow::Result<PgPool> {
    const MAX_POOL_CONNECTIONS: usize = 8;

    Config {
        host: Some(host.to_string()),
        port: Some(port),
        dbname: Some(db_name.to_string()),
        user: Some(user.to_string()),
        password: password.map(|p| p.to_string()),
        manager: Some(ManagerConfig {
            recycling_method: RecyclingMethod::Verified,
        }),
        pool: Some(PoolConfig {
            max_size: MAX_POOL_CONNECTIONS,
            ..Default::default()
        }),
        ..Default::default()
    }
    .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
    .with_context(|| format!("Failed to create database pool for: {host}:{port}/{db_name}"))
}

/// Creates a PostgreSQL container with optimized settings for testing.
pub(crate) async fn create_postgres_container(
    image: &str,
    container_name: &str,
    user: &str,
    password: Option<&str>,
    database: &str,
) -> anyhow::Result<ContainerAsync<GenericImage>> {
    let (image_name, tag) = image.split_once(':').unwrap_or((image, "latest"));

    let mut container_image = GenericImage::new(image_name, tag)
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_container_name(container_name)
        .with_env_var("POSTGRES_USER", user)
        .with_env_var("POSTGRES_DB", database)
        .with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
        .with_env_var(
            "POSTGRES_INITDB_ARGS",
            "--encoding=UTF-8 --lc-collate=C --lc-ctype=C",
        )
        // Use custom settings to speed up testing
        .with_cmd(vec![
            "postgres",
            "-c",
            "fsync=off",
            "-c",
            "synchronous_commit=off",
            "-c",
            "full_page_writes=off",
            "-c",
            "shared_buffers=128MB",
            "-c",
            "max_connections=500",
            "-c",
            "client_min_messages=warning",
        ]);

    if let Some(pwd) = password {
        container_image = container_image.with_env_var("POSTGRES_PASSWORD", pwd);
    }

    container_image
        .start()
        .await
        .context("PostgreSQL container should start successfully")
}
