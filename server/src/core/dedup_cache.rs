use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// In-memory deduplication cache with TTL-based expiry.
///
/// On failure, callers should call `remove()` to allow Pub/Sub retries.
#[derive(Clone)]
pub struct DedupCache {
    cache: Arc<Mutex<HashMap<String, Instant>>>,
    ttl: Duration,
}

impl DedupCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    /// Returns `true` if the message is a duplicate (already being processed or succeeded).
    /// If not a duplicate, marks it as processing.
    pub async fn check_and_mark(&self, message_id: &str) -> bool {
        let mut cache = self.cache.lock().await;

        let now = Instant::now();
        let ttl = self.ttl;
        cache.retain(|_, timestamp| now.duration_since(*timestamp) < ttl);

        if cache.contains_key(message_id) {
            return true;
        }

        cache.insert(message_id.to_string(), now);
        false
    }

    /// Remove from cache to allow retry on processing failure.
    pub async fn remove(&self, message_id: &str) {
        let mut cache = self.cache.lock().await;
        cache.remove(message_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_and_mark_new_message() {
        let cache = DedupCache::new(600);
        assert!(!cache.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn test_check_and_mark_duplicate() {
        let cache = DedupCache::new(600);
        assert!(!cache.check_and_mark("msg-1").await);
        assert!(cache.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn test_remove_allows_retry() {
        let cache = DedupCache::new(600);
        assert!(!cache.check_and_mark("msg-1").await);
        cache.remove("msg-1").await;
        assert!(!cache.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let cache = DedupCache::new(0); // 0-second TTL = expires immediately
        assert!(!cache.check_and_mark("msg-1").await);
        // Next call cleans expired entries first
        assert!(!cache.check_and_mark("msg-1").await);
    }
}
