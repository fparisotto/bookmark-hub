use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{ensure, Result as AnyhowResult};
use clap::{Args, Parser};
use secrecy::SecretString;
use url::Url;

use self::db::PgPool;

pub mod auth_rate_limit;
pub mod bookmark_identity;
pub mod chrome_client;
pub mod daemon;
pub mod db;
pub mod endpoints;
pub mod error;
pub mod llm;
pub mod mcp;
pub mod rag;
pub mod readability;
pub mod tokenizer;

pub const TEXT_AI_PIPELINE_VERSION: i32 = 1;
pub const EMBEDDING_PIPELINE_VERSION: i32 = 1;

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

    /// Comma-separated list of allowed Host header values for the MCP
    /// Streamable HTTP endpoint. Defaults to localhost, 127.0.0.1, ::1
    /// (rmcp built-in defaults). Set to the external hostname(s) when
    /// deploying behind a reverse proxy or ingress.
    #[arg(long, env = "APP_MCP_ALLOWED_HOSTS")]
    pub mcp_allowed_hosts: Option<String>,

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

    #[arg(long, env = "AI_TEXT_CHUNK_SIZE")]
    pub ai_text_chunk_size: Option<usize>,

    #[arg(long, env = "AI_TEXT_CHUNK_OVERLAP")]
    pub ai_text_chunk_overlap: Option<usize>,

    #[arg(long, env = "AI_EMBED_CHUNK_SIZE", default_value = "2000")]
    pub ai_embed_chunk_size: usize,

    #[arg(long, env = "AI_EMBED_CHUNK_OVERLAP", default_value = "200")]
    pub ai_embed_chunk_overlap: usize,

    #[arg(long, env = "AI_TEXT_CLAIM_WINDOW_SECS", default_value = "1800")]
    pub ai_text_claim_window_secs: u64,

    #[arg(long, env = "AI_EMBED_CLAIM_WINDOW_SECS", default_value = "900")]
    pub ai_embed_claim_window_secs: u64,

    #[arg(long, env = "LLM_MAX_IN_FLIGHT_TOTAL", default_value = "4")]
    pub llm_max_in_flight_total: usize,

    #[arg(long, env = "LLM_MAX_IN_FLIGHT_BACKGROUND", default_value = "2")]
    pub llm_max_in_flight_background: usize,

    #[arg(long, env = "LLM_TEXT_RPM_INTERACTIVE")]
    pub llm_text_rpm_interactive: Option<u32>,

    #[arg(long, env = "LLM_TEXT_RPM_BACKGROUND")]
    pub llm_text_rpm_background: Option<u32>,

    #[arg(long, env = "LLM_EMBED_RPM_INTERACTIVE")]
    pub llm_embed_rpm_interactive: Option<u32>,

    #[arg(long, env = "LLM_EMBED_RPM_BACKGROUND")]
    pub llm_embed_rpm_background: Option<u32>,

    #[arg(long, env = "LLM_RETRY_BASE_DELAY_MS", default_value = "1000")]
    pub llm_retry_base_delay_ms: u64,

    #[arg(long, env = "LLM_RETRY_MAX_DELAY_MS", default_value = "30000")]
    pub llm_retry_max_delay_ms: u64,

    // Ollama-specific
    #[arg(long, env = "OLLAMA_URL")]
    pub ollama_url: Option<Url>,

    // Cloud provider API keys
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_api_key: Option<SecretString>,

    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<SecretString>,

    #[arg(long, env = "GEMINI_API_KEY")]
    pub gemini_api_key: Option<SecretString>,

    #[arg(long, env = "OPENROUTER_API_KEY")]
    pub openrouter_api_key: Option<SecretString>,

    /// API key for the embedding provider (if different from text provider)
    #[arg(long, env = "LLM_EMBEDDING_API_KEY")]
    pub llm_embedding_api_key: Option<SecretString>,

    /// HTTP request timeout in seconds for LLM calls
    #[arg(long, env = "LLM_REQUEST_TIMEOUT_SECS")]
    pub llm_request_timeout_secs: Option<u64>,
}

impl LlmParams {
    pub fn resolved_text_chunk_size(&self) -> usize {
        self.ai_text_chunk_size.unwrap_or_else(|| {
            if self.llm_provider == "ollama" {
                1_000
            } else {
                2_000
            }
        })
    }

    pub fn resolved_text_chunk_overlap(&self) -> usize {
        self.ai_text_chunk_overlap.unwrap_or_else(|| {
            if self.llm_provider == "ollama" {
                100
            } else {
                200
            }
        })
    }

    pub fn validate_runtime_settings(&self) -> AnyhowResult<()> {
        let text_chunk_size = self.resolved_text_chunk_size();
        let text_chunk_overlap = self.resolved_text_chunk_overlap();
        ensure!(
            text_chunk_size > 0,
            "AI_TEXT_CHUNK_SIZE must be greater than 0"
        );
        ensure!(
            text_chunk_overlap < text_chunk_size,
            "AI_TEXT_CHUNK_OVERLAP must be smaller than AI_TEXT_CHUNK_SIZE"
        );
        ensure!(
            self.ai_embed_chunk_size > 0,
            "AI_EMBED_CHUNK_SIZE must be greater than 0"
        );
        ensure!(
            self.ai_embed_chunk_overlap < self.ai_embed_chunk_size,
            "AI_EMBED_CHUNK_OVERLAP must be smaller than AI_EMBED_CHUNK_SIZE"
        );
        ensure!(
            self.llm_max_in_flight_total > 0,
            "LLM_MAX_IN_FLIGHT_TOTAL must be greater than 0"
        );
        ensure!(
            self.llm_max_in_flight_background <= self.llm_max_in_flight_total,
            "LLM_MAX_IN_FLIGHT_BACKGROUND must be less than or equal to LLM_MAX_IN_FLIGHT_TOTAL"
        );
        ensure!(
            self.ai_text_claim_window_secs > 0,
            "AI_TEXT_CLAIM_WINDOW_SECS must be greater than 0"
        );
        ensure!(
            self.ai_embed_claim_window_secs > 0,
            "AI_EMBED_CLAIM_WINDOW_SECS must be greater than 0"
        );
        ensure!(
            self.llm_retry_base_delay_ms > 0,
            "LLM_RETRY_BASE_DELAY_MS must be greater than 0"
        );
        ensure!(
            self.llm_retry_max_delay_ms >= self.llm_retry_base_delay_ms,
            "LLM_RETRY_MAX_DELAY_MS must be greater than or equal to LLM_RETRY_BASE_DELAY_MS"
        );
        Ok(())
    }
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
