use clap::Parser;
use secrecy::SecretString;
use sqlx::{Pool, Postgres};
use std::{net::SocketAddr, sync::Arc};
use strum_macros::EnumString;
use url::Url;

pub mod auth;
pub mod daemon;
pub mod db;
pub mod endpoints;
pub mod error;
pub mod readability;
pub mod s3;

#[derive(Clone)]
pub struct AppContext {
    pub db: Pool<Postgres>,
    pub config: Arc<Config>,
}

#[derive(Debug, Clone, EnumString)]
pub enum Env {
    PROD,
    DEV,
}

#[derive(Parser, Clone, Debug)]
#[command(version)]
pub struct Config {
    #[arg(long, env = "APP_ENV")]
    pub app_env: Env,

    #[arg(long, env = "HMAC_KEY")]
    pub hmac_key: SecretString,

    #[arg(long, env = "DB_POOL_SIZE")]
    pub database_connection_pool_size: u8,

    #[arg(long, env = "DB_URL")]
    pub database_url: SecretString,

    #[arg(long, env = "LOKI_URL")]
    pub loki_url: Option<Url>,

    #[arg(long, env = "READABILITY_URL")]
    pub readability_url: Url,

    #[arg(long, env = "S3_BUCKET")]
    pub s3_bucket: String,

    #[arg(long, env = "S3_ENDPOINT")]
    pub s3_endpoint: Url,

    #[arg(long, env = "S3_REGION")]
    pub s3_region: String,

    #[arg(long, env = "S3_SECRET_KEY")]
    pub s3_secret_key: SecretString,

    #[arg(long, env = "S3_ACCESS_KEY")]
    pub s3_access_key: SecretString,

    #[arg(long, env = "APP_BIND", default_value = "[::]:3000")]
    pub bind: SocketAddr,
}
