use crate::domain::pr::RateLimitStatus;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Debug)]
pub struct RateLimitTracker {
    remaining: AtomicU32,
    limit: AtomicU32,
    reset_at: AtomicU64,
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            remaining: AtomicU32::new(5000),
            limit: AtomicU32::new(5000),
            reset_at: AtomicU64::new(0),
        }
    }

    pub fn update(&self, status: &RateLimitStatus) {
        self.remaining.store(status.remaining, Ordering::SeqCst);
        self.limit.store(status.limit, Ordering::SeqCst);
        self.reset_at.store(status.reset_at, Ordering::SeqCst);
    }

    pub fn get_remaining(&self) -> u32 {
        self.remaining.load(Ordering::SeqCst)
    }

    pub fn get_status(&self) -> RateLimitStatus {
        RateLimitStatus {
            remaining: self.remaining.load(Ordering::SeqCst),
            limit: self.limit.load(Ordering::SeqCst),
            reset_at: self.reset_at.load(Ordering::SeqCst),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_tracker() {
        let tracker = RateLimitTracker::new();
        assert_eq!(tracker.get_remaining(), 5000);

        tracker.update(&RateLimitStatus { remaining: 100, limit: 5000, reset_at: 123_456 });

        assert_eq!(tracker.get_remaining(), 100);
        let status = tracker.get_status();
        assert_eq!(status.remaining, 100);
        assert_eq!(status.reset_at, 123_456);
    }
}
