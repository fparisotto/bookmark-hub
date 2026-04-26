use anyhow::Context;
use deadpool_postgres::{
    Config, GenericClient, ManagerConfig, PoolConfig, RecyclingMethod, Runtime,
};
use secrecy::ExposeSecret;
use tracing::{debug, info, warn};

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

const EMBEDDING_INDEX_NAME: &str = "idx_bookmark_chunk_embedding";

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

const SCHEMAS: [(i32, &str); 8] = [
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
    (
        6,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/6_ai_generation_retry_state.sql"
        )),
    ),
    (
        7,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/7_bookmark_identity.sql"
        )),
    ),
    (
        8,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schema/8_flexible_embeddings.sql"
        )),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingProfile {
    pub provider: String,
    pub model: String,
    pub dimensions: usize,
}

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

    if get_schema_version(pool).await? >= 7 {
        bookmark::ensure_canonical_url_support(pool).await?;
    }

    Ok(())
}

pub async fn reconcile_embedding_profile(
    pool: &PgPool,
    target: &EmbeddingProfile,
) -> anyhow::Result<()> {
    let client = pool.get().await?;
    let stored = get_embedding_profile(&client).await?;
    let chunk_count = bookmark_chunk_count(&client).await?;

    match stored {
        Some(current) if current == *target => {
            info!(
                provider = %target.provider,
                model = %target.model,
                dimensions = target.dimensions,
                "Embedding profile matches current configuration"
            );
        }
        Some(current) => {
            warn!(
                old_provider = %current.provider,
                old_model = %current.model,
                old_dimensions = current.dimensions,
                new_provider = %target.provider,
                new_model = %target.model,
                new_dimensions = target.dimensions,
                "Embedding profile changed, clearing chunk embeddings"
            );
            client.execute("TRUNCATE bookmark_chunk", &[]).await?;
            upsert_embedding_profile(&client, target).await?;
        }
        None if chunk_count > 0 => {
            warn!(
                chunk_count,
                provider = %target.provider,
                model = %target.model,
                dimensions = target.dimensions,
                "Embedding profile metadata missing for existing chunks, clearing chunk embeddings"
            );
            client.execute("TRUNCATE bookmark_chunk", &[]).await?;
            upsert_embedding_profile(&client, target).await?;
        }
        None => {
            info!(
                provider = %target.provider,
                model = %target.model,
                dimensions = target.dimensions,
                "Initializing embedding profile metadata"
            );
            upsert_embedding_profile(&client, target).await?;
        }
    }

    ensure_embedding_index(&client, target.dimensions).await?;
    Ok(())
}

async fn get_embedding_profile(
    client: &impl GenericClient,
) -> anyhow::Result<Option<EmbeddingProfile>> {
    let row = client
        .query_opt(
            "SELECT provider, model, dimensions
             FROM embedding_config
             WHERE embedding_config_id = TRUE",
            &[],
        )
        .await?;

    Ok(row.map(|row| EmbeddingProfile {
        provider: row.get("provider"),
        model: row.get("model"),
        dimensions: row.get::<_, i32>("dimensions") as usize,
    }))
}

async fn upsert_embedding_profile(
    client: &impl GenericClient,
    profile: &EmbeddingProfile,
) -> anyhow::Result<()> {
    client
        .execute(
            "INSERT INTO embedding_config (embedding_config_id, provider, model, dimensions)
             VALUES (TRUE, $1, $2, $3)
             ON CONFLICT (embedding_config_id)
             DO UPDATE SET
                provider = EXCLUDED.provider,
                model = EXCLUDED.model,
                dimensions = EXCLUDED.dimensions,
                updated_at = NOW()",
            &[
                &profile.provider,
                &profile.model,
                &(profile.dimensions as i32),
            ],
        )
        .await?;
    Ok(())
}

async fn bookmark_chunk_count(client: &impl GenericClient) -> anyhow::Result<i64> {
    Ok(client
        .query_one("SELECT COUNT(*) FROM bookmark_chunk", &[])
        .await?
        .get(0))
}

async fn ensure_embedding_index(
    client: &impl GenericClient,
    dimensions: usize,
) -> anyhow::Result<()> {
    let expected_fragment = format!("vector({dimensions})");
    let existing = client
        .query_opt(
            "SELECT indexdef
             FROM pg_indexes
             WHERE schemaname = current_schema()
               AND tablename = 'bookmark_chunk'
               AND indexname = $1",
            &[&EMBEDDING_INDEX_NAME],
        )
        .await?;

    let needs_rebuild = match existing {
        Some(row) => {
            let definition: String = row.get("indexdef");
            !definition.contains(&expected_fragment)
        }
        None => true,
    };

    if needs_rebuild {
        warn!(
            index = EMBEDDING_INDEX_NAME,
            dimensions, "Rebuilding embedding ANN index"
        );
        let statement = format!(
            "DROP INDEX IF EXISTS {EMBEDDING_INDEX_NAME};
             CREATE INDEX {EMBEDDING_INDEX_NAME} ON bookmark_chunk
             USING hnsw (((embedding)::vector({dimensions})) vector_cosine_ops)
             WITH (m = 16, ef_construction = 64);"
        );
        client.batch_execute(&statement).await?;
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
