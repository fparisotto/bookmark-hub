use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthRateLimitKey {
    action: &'static str,
    client_ip: String,
    username: String,
}

impl AuthRateLimitKey {
    pub fn new(action: &'static str, client_ip: String, username: String) -> Self {
        Self {
            action,
            client_ip,
            username,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub retry_after_secs: u64,
}

#[derive(Debug)]
struct AttemptWindow {
    started_at: Instant,
    attempts: u32,
}

#[derive(Debug)]
pub struct AuthRateLimiter {
    max_attempts: u32,
    window: Duration,
    attempts: Mutex<HashMap<AuthRateLimitKey, AttemptWindow>>,
}

impl AuthRateLimiter {
    pub fn new(max_attempts: u32, window: Duration) -> Self {
        Self {
            max_attempts,
            window,
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: AuthRateLimitKey) -> RateLimitDecision {
        let mut attempts = self.attempts.lock().expect("auth limiter mutex poisoned");
        let now = Instant::now();

        attempts.retain(|_, window| now.duration_since(window.started_at) < self.window);

        let window = attempts.entry(key).or_insert(AttemptWindow {
            started_at: now,
            attempts: 0,
        });

        if now.duration_since(window.started_at) >= self.window {
            window.started_at = now;
            window.attempts = 0;
        }

        if window.attempts >= self.max_attempts {
            let retry_after_secs = self
                .window
                .saturating_sub(now.duration_since(window.started_at))
                .as_secs()
                .max(1);
            return RateLimitDecision {
                allowed: false,
                retry_after_secs,
            };
        }

        window.attempts += 1;

        RateLimitDecision {
            allowed: true,
            retry_after_secs: 0,
        }
    }

    pub fn reset(&self, key: &AuthRateLimitKey) {
        let mut attempts = self.attempts.lock().expect("auth limiter mutex poisoned");
        attempts.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{AuthRateLimitKey, AuthRateLimiter};

    #[test]
    fn blocks_after_max_attempts() {
        let limiter = AuthRateLimiter::new(2, Duration::from_secs(60));
        let key = AuthRateLimitKey::new("sign-in", "127.0.0.1".into(), "alice".into());

        assert!(limiter.check(key.clone()).allowed);
        assert!(limiter.check(key.clone()).allowed);

        let decision = limiter.check(key);
        assert!(!decision.allowed);
        assert!(decision.retry_after_secs > 0);
    }

    #[test]
    fn reset_clears_window() {
        let limiter = AuthRateLimiter::new(1, Duration::from_secs(60));
        let key = AuthRateLimitKey::new("sign-in", "127.0.0.1".into(), "alice".into());

        assert!(limiter.check(key.clone()).allowed);
        assert!(!limiter.check(key.clone()).allowed);

        limiter.reset(&key);

        assert!(limiter.check(key).allowed);
    }
}
