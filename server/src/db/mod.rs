use anyhow::Context;
use deadpool_postgres::{
    Config, GenericClient, ManagerConfig, PoolConfig, RecyclingMethod, Runtime,
};
use secrecy::ExposeSecret;
use tracing::{debug, info};

use crate::error::{Error, Result};
use crate::PgParams;

pub mod bookmark;
pub mod bookmark_task;
pub mod chunks;
pub mod rag;
pub mod search;
pub mod user;

pub type PgPool = deadpool_postgres::Pool;
pub type PgConnection = deadpool_postgres::Object;

const CREATE_GET_SCHEMA_FUNCTION: &str = "
CREATE OR REPLACE FUNCTION get_schema_version() RETURNS INTEGER AS $$
DECLARE
    max_version INTEGER;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_catalog.pg_tables
        WHERE schemaname = current_schema()
        AND tablename = 'schema_version'
    ) THEN
        SELECT COALESCE(MAX(version), 0) INTO max_version
        FROM schema_version;
    ELSE
        max_version := 0;
    END IF;
    RETURN max_version;
END;
$$ LANGUAGE plpgsql;";

const SCHEMAS: [(i32, &str); 5] = [
    (
        1,
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/1_unified.sql")),
    ),
    (
        2,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/2_embedding_dimension.sql"
        )),
    ),
    (
        3,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/3_lowercase_tags.sql"
        )),
    ),
    (
        4,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/4_trim_tags.sql"
        )),
    ),
    (
        5,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/5_reembed_qwen3.sql"
        )),
    ),
];

pub async fn get_pool(pg: PgParams) -> anyhow::Result<PgPool> {
    info!(
        pg_host = %pg.pg_host,
        pg_port = %pg.pg_port,
        max_connections = %pg.pg_max_connections,
        "Creating postgres pool"
    );
    debug!(user = "[REDACTED]", database = "[REDACTED]", "Pool config");

    let mut cfg = Config::new();
    cfg.host = Some(pg.pg_host.clone());
    cfg.port = Some(pg.pg_port);
    cfg.user = Some(pg.pg_user.expose_secret().to_owned());
    cfg.password = Some(pg.pg_password.expose_secret().to_owned());
    cfg.dbname = Some(pg.pg_database.expose_secret().to_owned());
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg.pool = Some(PoolConfig {
        max_size: pg.pg_max_connections as usize,
        ..Default::default()
    });

    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
        .with_context(|| format!("Failure creating postgres pool with params: {pg:?}"))?;

    info!(max_connections = %pg.pg_max_connections, "Postgres pool created successfully");
    Ok(pool)
}

async fn get_schema_version(pool: &PgPool) -> Result<i32> {
    debug!("Fetching current database schema version");
    let client = pool.get().await?;
    client.execute(CREATE_GET_SCHEMA_FUNCTION, &[]).await?;
    let schema_version = client.query_one("SELECT get_schema_version()", &[]).await?;
    let schema_version: i32 = schema_version.get(0);
    debug!(schema_version = %schema_version, "Current schema version retrieved");
    Ok(schema_version)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    info!("Starting database migrations check");
    let mut migrations_applied = 0;

    for (version, statement) in SCHEMAS {
        let schema_version: i32 = get_schema_version(pool).await?;
        if version > schema_version {
            info!(
                from_version = %schema_version,
                to_version = %version,
                "Applying migration"
            );
            let start = std::time::Instant::now();
            pool.get().await?.batch_execute(statement).await?;
            let elapsed = start.elapsed();
            info!(
                version = %version,
                elapsed = ?elapsed,
                "Migration applied successfully"
            );
            migrations_applied += 1;
        } else {
            debug!(version = %version, "Migration already applied, skipping");
        }
    }

    if migrations_applied > 0 {
        info!(migrations_applied = %migrations_applied, "Applied migrations successfully");
    } else {
        info!("Database schema is up to date");
    }
    Ok(())
}

/// Check the current embedding vector dimension and adjust if needed.
/// If the dimension changed, truncates all chunks (they'll be re-embedded by
/// the daemon).
pub async fn ensure_embedding_dimension(pool: &PgPool, target_dim: usize) -> anyhow::Result<()> {
    let client = pool.get().await?;

    // Query current dimension from pg_attribute
    let row = client
        .query_opt(
            "SELECT atttypmod FROM pg_attribute
             WHERE attrelid = 'bookmark_chunk'::regclass
             AND attname = 'embedding'",
            &[],
        )
        .await?;

    let current_dim: Option<i32> = row.map(|r| r.get(0));

    match current_dim {
        Some(dim) if dim > 0 && dim as usize == target_dim => {
            info!(
                dimension = target_dim,
                "Embedding dimension matches, no change needed"
            );
        }
        Some(dim) if dim > 0 => {
            tracing::warn!(
                current = dim,
                target = target_dim,
                "Embedding dimension changed, truncating chunks and altering column"
            );
            let stmt = format!(
                "TRUNCATE bookmark_chunk;
                 ALTER TABLE bookmark_chunk ALTER COLUMN embedding TYPE VECTOR({target_dim});"
            );
            client.batch_execute(&stmt).await?;
            info!(
                new_dimension = target_dim,
                "Embedding dimension updated, all bookmarks will be re-embedded"
            );
        }
        _ => {
            debug!("No embedding column dimension found (table may not exist yet), skipping check");
        }
    }

    Ok(())
}

pub async fn run_health_check(pool: &PgPool) -> Result<()> {
    debug!("Running database health check");
    let start = std::time::Instant::now();
    let client = pool.get().await?;
    let _ = client.query_one("SELECT 1", &[]).await?;
    let elapsed = start.elapsed();
    debug!(elapsed = ?elapsed, "Database health check completed");
    Ok(())
}

pub trait ResultExt<T> {
    fn on_constraint(
        self,
        name: &str,
        f: impl FnOnce(tokio_postgres::error::DbError) -> Error,
    ) -> Result<T, Error>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<Error>,
{
    fn on_constraint(
        self,
        name: &str,
        map_err: impl FnOnce(tokio_postgres::error::DbError) -> Error,
    ) -> Result<T, Error> {
        self.map_err(|e| {
            let into_error: Error = e.into();
            if let Error::DatabaseError(ref inner) = into_error {
                if let Some(db_error) = inner.as_db_error() {
                    if db_error.constraint() == Some(name) {
                        return map_err(db_error.clone());
                    }
                }
            }
            into_error
        })
    }
}
