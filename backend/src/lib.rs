use clap::Parser;
use secrecy::SecretString;
use sqlx::{Pool, Postgres};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use strum_macros::EnumString;
use url::Url;

pub mod auth;
pub mod daemon;
pub mod db;
pub mod endpoints;
pub mod error;
pub mod readability;

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

    #[arg(long, env = "APP_BIND", default_value = "[::]:3000")]
    pub bind: SocketAddr,

    #[arg(long, env = "APP_DATA_DIR")]
    pub data_dir: PathBuf,
}
