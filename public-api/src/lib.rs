use std::sync::Arc;

use auth::Keys;
use sqlx::{Pool, Postgres};

use anyhow::Context;

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
    pub fn parse() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("env DATABASE_URL is required")?;
        let database_connection_pool_size: u8 =
            std::env::var("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let hmac_key = std::env::var("HMAC_KEY").context("env HMAC_KEY is required")?;
        let auth_keys = Keys::new(hmac_key.as_bytes());
        Ok(Config {
            database_url,
            database_connection_pool_size,
            hmac_key,
            auth_keys,
        })
    }
}
