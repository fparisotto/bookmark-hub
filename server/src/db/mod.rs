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

const SCHEMAS: [(i32, &str); 1] = [(
    1,
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/1_init.sql")),
)];

pub async fn get_pool(pg: PgParams) -> anyhow::Result<PgPool> {
    info!("Creating postgres pool with {pg:?}");
    let mut cfg = Config::new();
    cfg.host = Some(pg.pg_host.clone());
    cfg.port = Some(pg.pg_port);
    cfg.user = Some(pg.pg_user.expose_secret().clone());
    cfg.password = Some(pg.pg_password.expose_secret().clone());
    cfg.dbname = Some(pg.pg_database.expose_secret().clone());
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
    debug!("Created postgres pool with {pg:?}");
    Ok(pool)
}

async fn get_schema_version(pool: &PgPool) -> Result<i32> {
    let client = pool.get().await?;
    client.execute(CREATE_GET_SCHEMA_FUNCTION, &[]).await?;
    let schema_version = client.query_one("SELECT get_schema_version()", &[]).await?;
    let schema_version: i32 = schema_version.get(0);
    Ok(schema_version)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    for (version, statement) in SCHEMAS {
        let schema_version: i32 = get_schema_version(pool).await?;
        debug!("Current schema version: {schema_version}");
        if version > schema_version {
            debug!("Applying schema version: {version}");
            pool.get().await?.batch_execute(statement).await?;
        }
    }
    Ok(())
}

pub async fn run_health_check(pool: &PgPool) -> Result<()> {
    let client = pool.get().await?;
    let _ = client.query_one("SELECT 1", &[]).await?;
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
