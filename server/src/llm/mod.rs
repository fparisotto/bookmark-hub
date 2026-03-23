mod operations;
mod provider;

pub use operations::*;
pub use provider::build_llm_client;

use rig::providers::{anthropic, gemini, ollama, openai, openrouter};

/// Text completion client supporting multiple LLM providers.
#[derive(Clone)]
pub enum TextClient {
    Ollama(ollama::Client),
    OpenAI(openai::Client),
    Anthropic(anthropic::Client),
    Gemini(gemini::Client),
    OpenRouter(openrouter::Client),
}

/// Embedding client supporting providers with embedding APIs.
#[derive(Clone)]
pub enum EmbeddingClient {
    Ollama(ollama::Client),
    OpenAI(openai::Client),
    Gemini(gemini::Client),
}

/// Provider-agnostic LLM client supporting mixed text/embedding providers.
#[derive(Clone)]
pub struct LlmClient {
    pub text_client: TextClient,
    pub text_model: String,
    pub embedding_client: EmbeddingClient,
    pub embedding_model: String,
    pub embedding_ndims: usize,
}
