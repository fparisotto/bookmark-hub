use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser};
use secrecy::SecretString;
use url::Url;

use self::db::PgPool;

pub mod auth_rate_limit;
pub mod chrome_client;
pub mod daemon;
pub mod db;
pub mod endpoints;
pub mod error;
pub mod llm;
pub mod rag;
pub mod readability;
pub mod tokenizer;

#[derive(Clone)]
pub struct AppContext {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub auth_rate_limiter: Arc<auth_rate_limit::AuthRateLimiter>,
    pub tx_new_task: tokio::sync::watch::Sender<()>,
    pub llm_client: Option<llm::LlmClient>,
}

#[derive(Parser, Clone, Debug)]
#[command(version)]
pub struct Config {
    #[arg(long, env = "HMAC_KEY")]
    pub hmac_key: SecretString,

    #[clap(flatten)]
    pub pg: PgParams,

    #[clap(flatten)]
    pub llm: LlmParams,

    #[clap(flatten)]
    pub chrome: Option<ChromeParams>,

    #[arg(long, env = "APP_BIND", default_value = "[::]:3000")]
    pub bind: SocketAddr,

    #[arg(long, env = "APP_CORS_ALLOW_ORIGIN")]
    pub cors_allow_origin: Option<String>,

    #[arg(long, env = "APP_DATA_DIR")]
    pub data_dir: PathBuf,

    #[arg(long, env = "SPA_DIST")]
    pub spa_dir_dir: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct LlmParams {
    /// Text completion provider: ollama, openai, anthropic, gemini, openrouter
    #[arg(long, env = "LLM_PROVIDER", default_value = "ollama")]
    pub llm_provider: String,

    /// Embedding provider (defaults to LLM_PROVIDER if unset)
    #[arg(long, env = "LLM_EMBEDDING_PROVIDER")]
    pub llm_embedding_provider: Option<String>,

    #[arg(long, env = "LLM_TEXT_MODEL")]
    pub llm_text_model: Option<String>,

    #[arg(long, env = "LLM_EMBEDDING_MODEL")]
    pub llm_embedding_model: Option<String>,

    #[arg(long, env = "LLM_EMBEDDING_DIMENSION")]
    pub llm_embedding_dimension: Option<usize>,

    // Ollama-specific
    #[arg(long, env = "OLLAMA_URL")]
    pub ollama_url: Option<Url>,

    // Cloud provider API keys
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<String>,

    #[arg(long, env = "GEMINI_API_KEY")]
    pub gemini_api_key: Option<String>,

    #[arg(long, env = "OPENROUTER_API_KEY")]
    pub openrouter_api_key: Option<String>,

    /// API key for the embedding provider (if different from text provider)
    #[arg(long, env = "LLM_EMBEDDING_API_KEY")]
    pub llm_embedding_api_key: Option<String>,

    /// HTTP request timeout in seconds for LLM calls
    #[arg(long, env = "LLM_REQUEST_TIMEOUT_SECS")]
    pub llm_request_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Args)]
pub struct ChromeParams {
    #[arg(long, env = "CHROME_HOST")]
    pub chrome_host: String,

    #[arg(long, env = "CHROME_PORT", default_value = "9222")]
    pub chrome_port: u16,
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
