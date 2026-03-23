use std::time::Duration;

use anyhow::{bail, Result};
use rig::client::Nothing;
use rig::providers::{anthropic, gemini, ollama, openai, openrouter};

use super::{EmbeddingClient, LlmClient, TextClient};
use crate::LlmParams;

/// Strip trailing slash from URL to avoid double-slash in rig-core's URL construction.
fn ollama_base_url(params: &LlmParams) -> String {
    params
        .ollama_url
        .as_ref()
        .map(|u| u.as_str().trim_end_matches('/').to_string())
        .unwrap_or_else(|| "http://localhost:11434".to_string())
}

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

fn http_client(params: &LlmParams) -> Result<reqwest::Client> {
    let timeout = params
        .llm_request_timeout_secs
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT);
    Ok(reqwest::Client::builder().timeout(timeout).build()?)
}

/// Build an LlmClient from configuration params.
/// Returns None if no text model is configured (AI features disabled).
pub fn build_llm_client(params: &LlmParams) -> Result<Option<LlmClient>> {
    let text_model = match &params.llm_text_model {
        Some(m) => m.clone(),
        None => return Ok(None),
    };

    let text_client = build_text_client(params)?;

    let emb_provider = params
        .llm_embedding_provider
        .as_deref()
        .unwrap_or(&params.llm_provider);
    let embedding_model = params
        .llm_embedding_model
        .clone()
        .unwrap_or_else(|| text_model.clone());
    let embedding_ndims = params.llm_embedding_dimension.unwrap_or(1024);
    let embedding_client = build_embedding_client(emb_provider, params)?;

    Ok(Some(LlmClient {
        text_client,
        text_model,
        embedding_client,
        embedding_model,
        embedding_ndims,
    }))
}

fn build_text_client(params: &LlmParams) -> Result<TextClient> {
    match params.llm_provider.as_str() {
        "ollama" => {
            let url = ollama_base_url(params);
            let client = ollama::Client::builder()
                .base_url(&url)
                .http_client(http_client(params)?)
                .api_key(Nothing)
                .build()?;
            Ok(TextClient::Ollama(client))
        }
        "openai" => {
            let key = params
                .openai_api_key
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY required for openai provider"))?;
            let client = openai::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::OpenAI(client))
        }
        "anthropic" => {
            let key = params.anthropic_api_key.as_deref().ok_or_else(|| {
                anyhow::anyhow!("ANTHROPIC_API_KEY required for anthropic provider")
            })?;
            let client = anthropic::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::Anthropic(client))
        }
        "gemini" => {
            let key = params
                .gemini_api_key
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("GEMINI_API_KEY required for gemini provider"))?;
            let client = gemini::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::Gemini(client))
        }
        "openrouter" => {
            let key = params.openrouter_api_key.as_deref().ok_or_else(|| {
                anyhow::anyhow!("OPENROUTER_API_KEY required for openrouter provider")
            })?;
            let client = openrouter::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::OpenRouter(client))
        }
        other => bail!("Unknown LLM_PROVIDER: {other}"),
    }
}

fn build_embedding_client(provider: &str, params: &LlmParams) -> Result<EmbeddingClient> {
    // For embedding provider, use LLM_EMBEDDING_API_KEY if set, otherwise fall back
    // to the main provider's API key.
    match provider {
        "ollama" => {
            let url = ollama_base_url(params);
            let client = ollama::Client::builder()
                .base_url(&url)
                .http_client(http_client(params)?)
                .api_key(Nothing)
                .build()?;
            Ok(EmbeddingClient::Ollama(client))
        }
        "openai" => {
            let key = params
                .llm_embedding_api_key
                .as_deref()
                .or(params.openai_api_key.as_deref())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "OPENAI_API_KEY or LLM_EMBEDDING_API_KEY required for openai embeddings"
                    )
                })?;
            let client = openai::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(EmbeddingClient::OpenAI(client))
        }
        "gemini" => {
            let key = params
                .llm_embedding_api_key
                .as_deref()
                .or(params.gemini_api_key.as_deref())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "GEMINI_API_KEY or LLM_EMBEDDING_API_KEY required for gemini embeddings"
                    )
                })?;
            let client = gemini::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(EmbeddingClient::Gemini(client))
        }
        other => bail!(
            "Provider '{other}' does not support embeddings. Use LLM_EMBEDDING_PROVIDER to select ollama, openai, or gemini for embeddings."
        ),
    }
}
