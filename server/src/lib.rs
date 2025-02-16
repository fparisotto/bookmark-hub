use clap::{Args, Parser};
use secrecy::SecretString;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use url::Url;

use self::db::PgPool;

pub mod daemon;
pub mod db;
pub mod endpoints;
pub mod error;
pub mod ollama;
pub mod readability;
pub mod tokenizer;

#[derive(Clone)]
pub struct AppContext {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub tx_new_task: tokio::sync::watch::Sender<()>,
}

#[derive(Parser, Clone, Debug)]
#[command(version)]
pub struct Config {
    #[arg(long, env = "HMAC_KEY")]
    pub hmac_key: SecretString,

    #[clap(flatten)]
    pub pg: PgParams,

    #[clap(flatten)]
    pub ollama: OllamaParams,

    #[arg(long, env = "READABILITY_URL")]
    pub readability_url: Url,

    #[arg(long, env = "APP_BIND", default_value = "[::]:3000")]
    pub bind: SocketAddr,

    #[arg(long, env = "APP_DATA_DIR")]
    pub data_dir: PathBuf,

    #[arg(long, env = "SPA_DIST")]
    pub spa_dir_dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct OllamaParams {
    #[arg(long, env = "OLLAMA_URL")]
    pub ollama_url: Option<Url>,

    #[arg(long, env = "OLLAMA_TEXT_MODEL")]
    pub ollama_text_model: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct PgParams {
    #[clap(long, help = "Postgres host", env = "PG_HOST")]
    pg_host: String,

    #[clap(long, help = "Postgres port", env = "PG_PORT")]
    pg_port: u16,

    #[clap(long, help = "Postgres user", env = "PG_USER")]
    pg_user: SecretString,

    #[clap(long, help = "Postgres password", env = "PG_PASSWORD")]
    pg_password: SecretString,

    #[clap(long, help = "Postgres database", env = "PG_DATABASE")]
    pg_database: SecretString,

    #[clap(
        long,
        help = "Postgres connection pool max connections",
        env = "PG_MAX_CONNECTIONS"
    )]
    pg_max_connections: u8,
}
