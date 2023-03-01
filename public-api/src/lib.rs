use anyhow::{Context, Result};
use std::sync::Arc;

use auth::Keys;
use sqlx::{Pool, Postgres};

pub mod auth;
pub mod database;
pub mod endpoints;
pub mod error;

#[derive(Clone)]
pub struct AppContext {
    pub db: Pool<Postgres>,
    pub config: Arc<Config>,
}

pub struct Config {
    pub database_url: String,
    pub database_connection_pool_size: u8,
    pub hmac_key: String,
    pub auth_keys: Keys,
}

impl Config {
    fn env(key: &str) -> Result<String> {
        std::env::var(key).context(format!("missing env {key}"))
    }

    pub fn parse() -> anyhow::Result<Self> {
        let database_url = Config::env("DATABASE_URL")?;
        let pool_size: u8 = Config::env("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let hmac_key = Config::env("HMAC_KEY")?;
        let auth_keys = Keys::new(hmac_key.as_bytes());
        Ok(Config {
            database_url,
            database_connection_pool_size: pool_size,
            hmac_key,
            auth_keys,
        })
    }
}
