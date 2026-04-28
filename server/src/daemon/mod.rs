use std::time::Duration;

use chrono::Duration as ChronoDuration;

use crate::LlmParams;

pub mod add_bookmark;
pub mod embeddings;
pub mod text_ai;

pub const DAEMON_IDLE_SLEEP: Duration = Duration::from_secs(300);
pub const AI_GENERATION_MAX_RETRIES: i16 = 5;

#[derive(Debug, Clone)]
pub struct AiDaemonSettings {
    pub text_chunk_size: usize,
    pub text_chunk_overlap: usize,
    pub embed_chunk_size: usize,
    pub embed_chunk_overlap: usize,
    pub text_claim_window: ChronoDuration,
    pub embed_claim_window: ChronoDuration,
}

impl AiDaemonSettings {
    pub fn from_llm_params(params: &LlmParams) -> anyhow::Result<Self> {
        params.validate_runtime_settings()?;
        Ok(Self {
            text_chunk_size: params.resolved_text_chunk_size(),
            text_chunk_overlap: params.resolved_text_chunk_overlap(),
            embed_chunk_size: params.ai_embed_chunk_size,
            embed_chunk_overlap: params.ai_embed_chunk_overlap,
            text_claim_window: ChronoDuration::seconds(params.ai_text_claim_window_secs as i64),
            embed_claim_window: ChronoDuration::seconds(params.ai_embed_claim_window_secs as i64),
        })
    }
}

pub fn ai_generation_backoff(attempt: i16) -> ChronoDuration {
    match attempt {
        1 => ChronoDuration::minutes(5),
        2 => ChronoDuration::minutes(15),
        3 => ChronoDuration::hours(1),
        4 => ChronoDuration::hours(6),
        _ => ChronoDuration::hours(24),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;

    use super::ai_generation_backoff;

    #[test]
    fn ai_generation_backoff_uses_step_schedule() {
        assert_eq!(ai_generation_backoff(1), ChronoDuration::minutes(5));
        assert_eq!(ai_generation_backoff(2), ChronoDuration::minutes(15));
        assert_eq!(ai_generation_backoff(3), ChronoDuration::hours(1));
        assert_eq!(ai_generation_backoff(4), ChronoDuration::hours(6));
        assert_eq!(ai_generation_backoff(5), ChronoDuration::hours(24));
        assert_eq!(ai_generation_backoff(8), ChronoDuration::hours(24));
    }
}
