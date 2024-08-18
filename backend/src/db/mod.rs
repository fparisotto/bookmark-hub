use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;

use crate::error::{Error, Result};
use crate::Config;

pub mod bookmark;
pub mod search;
pub mod task;
pub mod user;

pub async fn connect(config: &Config) -> Result<sqlx::Pool<sqlx::Postgres>> {
    let db: sqlx::Pool<sqlx::Postgres> = PgPoolOptions::new()
        .max_connections(config.database_connection_pool_size as u32)
        .connect(config.database_url.expose_secret())
        .await?;
    Ok(db)
}

pub async fn run_migrations(db: &sqlx::Pool<sqlx::Postgres>) -> Result<()> {
    sqlx::migrate!().run(db).await?;
    Ok(())
}

pub trait ResultExt<T> {
    fn on_constraint(
        self,
        name: &str,
        f: impl FnOnce(Box<dyn sqlx::error::DatabaseError>) -> Error,
    ) -> Result<T, Error>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<Error>,
{
    fn on_constraint(
        self,
        name: &str,
        map_err: impl FnOnce(Box<dyn sqlx::error::DatabaseError>) -> Error,
    ) -> Result<T, Error> {
        self.map_err(|e| match e.into() {
            Error::Database(sqlx::Error::Database(dbe)) if dbe.constraint() == Some(name) => {
                map_err(dbe)
            }
            e => e,
        })
    }
}
