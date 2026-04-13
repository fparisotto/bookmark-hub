use std::time::Duration;

use chrono::Duration as ChronoDuration;

pub mod add_bookmark;
pub mod embeddings;
pub mod summary;
pub mod tag;

pub const DAEMON_IDLE_SLEEP: Duration = Duration::from_secs(300);
pub const TOKENIZER_WINDOW_SIZE: usize = 1_000;
pub const TOKENIZER_WINDOW_OVERLAP: usize = 100;
pub const AI_GENERATION_MAX_RETRIES: i16 = 5;

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
