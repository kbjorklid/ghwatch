use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::domain::pr::RateLimitStatus;

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
    pub fn new() -> Self {
        Self {
            remaining: AtomicU32::new(5000),
            limit: AtomicU32::new(5000),
            reset_at: AtomicU64::new(0),
        }
    }

    pub fn update(&self, status: RateLimitStatus) {
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
