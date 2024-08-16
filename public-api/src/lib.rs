use anyhow::{Context, Result};
use secrecy::{Secret, SecretString};
use std::str::FromStr;
use std::sync::Arc;
use strum_macros::EnumString;
use url::Url;

use auth::Keys;
use sqlx::{Pool, Postgres};

pub mod auth;
pub mod daemon;
pub mod database;
pub mod endpoints;
pub mod error;
pub mod readability;

#[derive(Clone)]
pub struct AppContext {
    pub db: Pool<Postgres>,
    pub config: Arc<Config>,
}

#[derive(Debug, EnumString)]
pub enum Env {
    PROD,
    DEV,
}

#[derive(Debug)]
pub struct Config {
    pub app_env: Env,
    pub auth_keys: Secret<Keys>,
    pub database_connection_pool_size: u8,
    pub database_url: SecretString,
    pub external_s3_endpoint: String,
    pub loki_url: Option<Url>,
    pub readability_endpoint: String,
    pub s3_access_key: String,
    pub s3_bucket: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_secret_key: String,
}

impl Config {
    fn required_env(key: &str) -> Result<String> {
        std::env::var(key).context(format!("missing env {key}"))
    }

    pub fn parse() -> anyhow::Result<Self> {
        let app_env: Env = Env::from_str(&Config::required_env("APP_ENV")?)?;
        let hmac_key = Config::required_env("HMAC_KEY")?;
        let auth_keys = Keys::new(hmac_key.as_bytes());
        let s3_access_key = Config::required_env("S3_ACCESS_KEY")?;
        let s3_secret_key = Config::required_env("S3_SECRET_KEY")?;
        let s3_endpoint = Config::required_env("S3_ENDPOINT")?;
        let s3_region = Config::required_env("S3_REGION")?;
        let s3_bucket = Config::required_env("S3_BUCKET")?;
        let readability_endpoint = Config::required_env("READABILITY_ENDPOINT")?;
        let database_url = Config::required_env("DATABASE_URL")?;
        let database_connection_pool_size: u8 =
            Config::required_env("DATABASE_CONNECTION_POOL_SIZE")?.parse()?;
        let external_s3_endpoint = Config::required_env("EXTERNAL_S3_ENDPOINT")?;
        let loki_url = std::env::var("LOKI_URL")
            .ok()
            .and_then(|url| Url::parse(&url).ok());
        Ok(Config {
            app_env,
            auth_keys: auth_keys.into(),
            loki_url,
            s3_access_key,
            s3_secret_key,
            s3_endpoint,
            s3_region,
            s3_bucket,
            readability_endpoint,
            database_url: database_url.into(),
            database_connection_pool_size,
            external_s3_endpoint,
        })
    }
}
