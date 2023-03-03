use anyhow::{Context, Result};
use std::str::FromStr;
use std::sync::Arc;
use strum_macros::EnumString;
use url::Url;

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

#[derive(EnumString)]
pub enum Env {
    PROD,
    DEV,
}

pub struct Config {
    pub app_env: Env,
    pub database_url: String,
    pub database_connection_pool_size: u8,
    pub hmac_key: String,
    pub auth_keys: Keys,
    pub loki_url: Option<Url>,
}

impl Config {
    fn required_env(key: &str) -> Result<String> {
        std::env::var(key).context(format!("missing env {key}"))
    }

    pub fn parse() -> anyhow::Result<Self> {
        let app_env: Env = Env::from_str(&Config::required_env("APP_ENV")?)?;
        let database_url = Config::required_env("DATABASE_URL")?;
        let pool_size: u8 = Config::required_env("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let hmac_key = Config::required_env("HMAC_KEY")?;
        let auth_keys = Keys::new(hmac_key.as_bytes());
        let loki_url = std::env::var("LOKI_URL")
            .ok()
            .and_then(|url| Url::parse(&url).ok());
        Ok(Config {
            app_env,
            database_url,
            database_connection_pool_size: pool_size,
            hmac_key,
            auth_keys,
            loki_url,
        })
    }
}
