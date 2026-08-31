use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ViewBoostRegistry {
    inner: Arc<RwLock<HashMap<i64, (Vec<i64>, Instant)>>>,
}

impl ViewBoostRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn record(&self, site_id: i64, ids: Vec<i64>) {
        self.inner
            .write()
            .expect("view boost registry lock poisoned")
            .insert(site_id, (ids, Instant::now()));
    }

    pub fn current_ids(&self, ttl: Duration) -> Vec<i64> {
        let now = Instant::now();
        let mut registry = self
            .inner
            .write()
            .expect("view boost registry lock poisoned");
        registry.retain(|_, (_, recorded_at)| now.duration_since(*recorded_at) <= ttl);

        registry
            .values()
            .flat_map(|(ids, _)| ids.iter().copied())
            .collect()
    }
}

impl Default for ViewBoostRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ViewBoostRegistry;
    use std::time::Duration;

    #[test]
    fn record_returns_ids_until_they_expire() {
        let registry = ViewBoostRegistry::new();
        registry.record(1, vec![10, 11]);

        assert_eq!(registry.current_ids(Duration::from_secs(1)), vec![10, 11]);
        assert!(registry.current_ids(Duration::ZERO).is_empty());
    }

    #[test]
    fn current_ids_combines_multiple_sites() {
        let registry = ViewBoostRegistry::new();
        registry.record(1, vec![10]);
        registry.record(2, vec![20, 21]);

        let mut ids = registry.current_ids(Duration::from_secs(1));
        ids.sort_unstable();
        assert_eq!(ids, vec![10, 20, 21]);
    }
}
