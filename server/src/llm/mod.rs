mod operations;
mod provider;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
pub use operations::*;
pub use provider::build_llm_client;
use rand::Rng;
use reqwest::StatusCode;
use rig::providers::{anthropic, gemini, ollama, openai, openrouter};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;
use tracing::warn;

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

#[derive(Debug, Clone, Copy)]
pub enum LlmWorkClass {
    Interactive,
    Background,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LlmRequestKind {
    Text,
    Embedding,
}

#[derive(Clone)]
pub(crate) struct RetrySettings {
    base_delay: Duration,
    max_delay: Duration,
    max_attempts: usize,
}

struct RequestPermit {
    _total: OwnedSemaphorePermit,
    _background: Option<OwnedSemaphorePermit>,
}

struct RateGate {
    rpm: Option<u32>,
    next_allowed: tokio::sync::Mutex<Instant>,
}

impl RateGate {
    fn new(rpm: Option<u32>) -> Self {
        Self {
            rpm,
            next_allowed: tokio::sync::Mutex::new(Instant::now()),
        }
    }

    async fn wait_turn(&self) {
        let Some(rpm) = self.rpm else {
            return;
        };

        let interval = Duration::from_secs_f64(60.0 / rpm as f64);
        let release_at = {
            let mut next_allowed = self.next_allowed.lock().await;
            let now = Instant::now();
            let release_at = if *next_allowed > now {
                *next_allowed
            } else {
                now
            };
            *next_allowed = release_at + interval;
            release_at
        };
        let now = Instant::now();
        if release_at > now {
            tokio::time::sleep(release_at - now).await;
        }
    }
}

pub(crate) struct LlmLimiter {
    total: Arc<Semaphore>,
    background: Arc<Semaphore>,
    text_interactive: RateGate,
    text_background: RateGate,
    embed_interactive: RateGate,
    embed_background: RateGate,
}

impl LlmLimiter {
    pub(crate) fn new(params: &crate::LlmParams) -> Self {
        Self {
            total: Arc::new(Semaphore::new(params.llm_max_in_flight_total)),
            background: Arc::new(Semaphore::new(params.llm_max_in_flight_background)),
            text_interactive: RateGate::new(params.llm_text_rpm_interactive),
            text_background: RateGate::new(params.llm_text_rpm_background),
            embed_interactive: RateGate::new(params.llm_embed_rpm_interactive),
            embed_background: RateGate::new(params.llm_embed_rpm_background),
        }
    }

    async fn acquire(&self, class: LlmWorkClass, kind: LlmRequestKind) -> Result<RequestPermit> {
        let total = self.total.clone().acquire_owned().await?;
        let background = match class {
            LlmWorkClass::Interactive => None,
            LlmWorkClass::Background => Some(self.background.clone().acquire_owned().await?),
        };

        match (class, kind) {
            (LlmWorkClass::Interactive, LlmRequestKind::Text) => {
                self.text_interactive.wait_turn().await
            }
            (LlmWorkClass::Background, LlmRequestKind::Text) => {
                self.text_background.wait_turn().await
            }
            (LlmWorkClass::Interactive, LlmRequestKind::Embedding) => {
                self.embed_interactive.wait_turn().await
            }
            (LlmWorkClass::Background, LlmRequestKind::Embedding) => {
                self.embed_background.wait_turn().await
            }
        }

        Ok(RequestPermit {
            _total: total,
            _background: background,
        })
    }
}

/// Provider-agnostic LLM client supporting mixed text/embedding providers.
#[derive(Clone)]
pub struct LlmClient {
    pub text_provider: String,
    pub text_client: TextClient,
    pub text_model: String,
    pub embedding_provider: String,
    pub embedding_client: EmbeddingClient,
    pub embedding_model: String,
    pub embedding_ndims: usize,
    limiter: Arc<LlmLimiter>,
    retry: RetrySettings,
}

impl LlmClient {
    pub(crate) async fn run_with_retry<T, F, Fut>(
        &self,
        class: LlmWorkClass,
        kind: LlmRequestKind,
        operation_name: &str,
        make_call: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        for attempt in 0..=self.retry.max_attempts {
            let _permit = self.limiter.acquire(class, kind).await?;
            match make_call().await {
                Ok(value) => return Ok(value),
                Err(error)
                    if attempt < self.retry.max_attempts && is_retriable_llm_error(&error) =>
                {
                    let delay =
                        retry_delay(self.retry.base_delay, self.retry.max_delay, attempt as u32);
                    warn!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        delay_ms = delay.as_millis() as u64,
                        error = %error,
                        "Retrying transient LLM failure"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("retry loop must return before exhaustion")
    }
}

fn retry_delay(base: Duration, max: Duration, attempt: u32) -> Duration {
    let multiplier = 2u32.saturating_pow(attempt);
    let candidate = base.saturating_mul(multiplier);
    let upper = candidate.min(max);
    let upper_ms = upper.as_millis() as u64;
    let lower_ms = base.as_millis() as u64;
    let delay_ms = if upper_ms <= lower_ms {
        upper_ms
    } else {
        rand::thread_rng().gen_range(lower_ms..=upper_ms)
    };
    Duration::from_millis(delay_ms)
}

fn is_retriable_llm_error(error: &anyhow::Error) -> bool {
    for cause in error.chain() {
        if let Some(reqwest_error) = cause.downcast_ref::<reqwest::Error>() {
            if reqwest_error.is_timeout() || reqwest_error.is_connect() {
                return true;
            }
            if let Some(status) = reqwest_error.status() {
                if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
                    return true;
                }
            }
        }
    }

    let message = error.to_string().to_lowercase();
    message.contains("429")
        || message.contains("too many requests")
        || message.contains("timed out")
        || message.contains("timeout")
        || message.contains("connection reset")
        || message.contains("temporarily unavailable")
        || message.contains("server error")
}
