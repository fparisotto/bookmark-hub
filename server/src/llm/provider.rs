use std::time::Duration;

use anyhow::{bail, Result};
use rig::client::{EmbeddingsClient, Nothing};
use rig::providers::{anthropic, gemini, ollama, openai, openrouter};
use secrecy::ExposeSecret;

use super::{EmbeddingClient, LlmClient, RetrySettings, TextClient};
use crate::LlmParams;

/// Strip trailing slash from URL to avoid double-slash in rig-core's URL
/// construction.
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
pub async fn build_llm_client(params: &LlmParams) -> Result<Option<LlmClient>> {
    let text_model = match &params.llm_text_model {
        Some(m) => m.clone(),
        None => return Ok(None),
    };

    let text_client = build_text_client(params)?;
    let limiter = std::sync::Arc::new(super::LlmLimiter::new(params));

    let emb_provider = params
        .llm_embedding_provider
        .as_deref()
        .unwrap_or(&params.llm_provider);
    let embedding_model = params
        .llm_embedding_model
        .clone()
        .unwrap_or_else(|| text_model.clone());
    let embedding_client = build_embedding_client(emb_provider, params)?;
    let embedding_ndims =
        resolve_embedding_dimensions(params, emb_provider, &embedding_model, &embedding_client)
            .await?;

    Ok(Some(LlmClient {
        text_provider: params.llm_provider.clone(),
        text_client,
        text_model,
        embedding_provider: emb_provider.to_string(),
        embedding_client,
        embedding_model,
        embedding_ndims,
        limiter,
        retry: RetrySettings {
            base_delay: Duration::from_millis(params.llm_retry_base_delay_ms),
            max_delay: Duration::from_millis(params.llm_retry_max_delay_ms),
            max_attempts: 2,
        },
    }))
}

async fn resolve_embedding_dimensions(
    params: &LlmParams,
    provider: &str,
    model: &str,
    client: &EmbeddingClient,
) -> Result<usize> {
    if let Some(dimensions) = params.llm_embedding_dimension {
        return Ok(dimensions);
    }

    if let Some(dimensions) = infer_embedding_dimensions(provider, model) {
        return Ok(dimensions);
    }

    let probe = probe_embedding_dimensions(client, model).await?;
    Ok(probe.len())
}

async fn probe_embedding_dimensions(client: &EmbeddingClient, model: &str) -> Result<Vec<f32>> {
    use rig::embeddings::EmbeddingModel;

    let embedding = match client {
        EmbeddingClient::Ollama(c) => {
            c.embedding_model(model)
                .embed_text("dimension probe")
                .await?
        }
        EmbeddingClient::OpenAI(c) => {
            c.embedding_model(model)
                .embed_text("dimension probe")
                .await?
        }
        EmbeddingClient::Gemini(c) => {
            c.embedding_model(model)
                .embed_text("dimension probe")
                .await?
        }
    };
    Ok(embedding
        .vec
        .into_iter()
        .map(|value| value as f32)
        .collect())
}

fn infer_embedding_dimensions(provider: &str, model: &str) -> Option<usize> {
    match provider {
        "openai" => match model {
            "text-embedding-3-large" => Some(3072),
            "text-embedding-3-small" | "text-embedding-ada-002" => Some(1536),
            _ => None,
        },
        "gemini" => match model {
            "gemini-embedding-001" => Some(3072),
            "text-embedding-004" => Some(768),
            _ => None,
        },
        "ollama" => match model {
            "all-minilm" => Some(384),
            "nomic-embed-text" => Some(768),
            _ => None,
        },
        _ => None,
    }
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
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY required for openai provider"))?;
            let client = openai::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::OpenAI(client))
        }
        "anthropic" => {
            let key = params
                .anthropic_api_key
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .ok_or_else(|| {
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
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .ok_or_else(|| anyhow::anyhow!("GEMINI_API_KEY required for gemini provider"))?;
            let client = gemini::Client::builder()
                .api_key(key)
                .http_client(http_client(params)?)
                .build()?;
            Ok(TextClient::Gemini(client))
        }
        "openrouter" => {
            let key = params
                .openrouter_api_key
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .ok_or_else(|| {
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
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .or(params.openai_api_key.as_ref().map(ExposeSecret::expose_secret))
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
                .as_ref()
                .map(ExposeSecret::expose_secret)
                .or(params.gemini_api_key.as_ref().map(ExposeSecret::expose_secret))
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
