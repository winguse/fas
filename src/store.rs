use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{Notify, RwLock};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct User {
    pub sid: String,
    pub domain: String,
    pub approved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_ip: String,
    pub last_seen: DateTime<Utc>,
    pub user_agent: String,
    pub request_count: u64,
}

pub struct StoreInner {
    pub users: HashMap<String, User>,
    pub rate_limits: HashMap<String, Instant>,
    pub dirty: bool,
    pub last_save: Instant,
    pub data_file: PathBuf,
}

#[derive(Clone)]
pub struct Store {
    pub inner: Arc<RwLock<StoreInner>>,
    pub notify_save: Arc<Notify>,
}

impl Store {
    pub fn new(data_file: PathBuf) -> Self {
        Self {
            inner: Arc::new(RwLock::new(StoreInner {
                users: HashMap::new(),
                rate_limits: HashMap::new(),
                dirty: false,
                last_save: Instant::now() - Duration::from_secs(3600), // set to past so first save happens immediately if dirty
                data_file,
            })),
            notify_save: Arc::new(Notify::new()),
        }
    }

    /// Load store from file
    pub async fn load(&self) -> std::io::Result<()> {
        let data_file = {
            let inner = self.inner.read().await;
            inner.data_file.clone()
        };

        if !data_file.exists() {
            tracing::info!("No existing data file — starting fresh");
            return Ok(());
        }

        let content = match fs::read_to_string(&data_file).await {
            Ok(c) => c,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    tracing::info!("No existing data file — starting fresh");
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
        };

        let mut inner = self.inner.write().await;
        inner.users.clear();
        let mut count = 0;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(u) = serde_json::from_str::<User>(line) {
                inner.users.insert(u.sid.clone(), u);
                count += 1;
            }
        }
        tracing::info!("Loaded {} records from {:?}", count, data_file);
        Ok(())
    }

    /// Save dirty store to file
    pub async fn flush(&self) -> std::io::Result<()> {
        let (users, data_file) = {
            let mut inner = self.inner.write().await;
            if !inner.dirty {
                return Ok(());
            }
            inner.dirty = false;
            inner.last_save = Instant::now();
            (
                inner.users.values().cloned().collect::<Vec<User>>(),
                inner.data_file.clone(),
            )
        };

        if let Some(parent) = data_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut content = String::new();
        for u in users {
            if let Ok(line) = serde_json::to_string(&u) {
                content.push_str(&line);
                content.push('\n');
            }
        }

        fs::write(&data_file, content).await?;
        tracing::info!("Saved records to {:?}", data_file);
        Ok(())
    }

    /// Mark store as dirty and trigger immediate save if throttle window has passed
    pub async fn mark_dirty(&self, save_interval: Duration) {
        let mut inner = self.inner.write().await;
        inner.dirty = true;
        if inner.last_save.elapsed() >= save_interval {
            self.notify_save.notify_one();
        }
    }

    /// Purge expired records and unapproved records
    pub async fn purge_old_records(&self, record_ttl: Duration, unapproved_ttl: Duration) -> usize {
        let mut to_delete = Vec::new();
        let now_utc = Utc::now();

        {
            let inner = self.inner.read().await;
            for (sid, user) in inner.users.iter() {
                let elapsed_created = now_utc.signed_duration_since(user.created_at);

                let rec_ttl_chrono = chrono::Duration::from_std(record_ttl)
                    .unwrap_or_else(|_| chrono::Duration::max_value());
                let unapp_ttl_chrono = chrono::Duration::from_std(unapproved_ttl)
                    .unwrap_or_else(|_| chrono::Duration::max_value());

                if elapsed_created >= rec_ttl_chrono
                    || (!user.approved && elapsed_created >= unapp_ttl_chrono)
                {
                    to_delete.push(sid.clone());
                }
            }
        }

        if to_delete.is_empty() {
            return 0;
        }

        let deleted_count = to_delete.len();
        {
            let mut inner = self.inner.write().await;
            for sid in &to_delete {
                inner.users.remove(sid);
            }
            inner.dirty = true;
        }

        deleted_count
    }

    /// Rate limit check: returns Err(retry_after_seconds) if rate limited
    pub async fn check_rate_limit(&self, ip: &str, window: Duration) -> Result<(), u64> {
        let mut inner = self.inner.write().await;
        let now = Instant::now();
        if let Some(&last) = inner.rate_limits.get(ip) {
            if now.duration_since(last) < window {
                let elapsed = now.duration_since(last);
                let remaining = window.saturating_sub(elapsed);
                let retry_after = remaining.as_secs_f64().ceil() as u64;
                return Err(retry_after.max(1));
            }
        }
        inner.rate_limits.insert(ip.to_string(), now);
        Ok(())
    }

    /// Clean up rate limit map entries older than the window
    pub async fn cleanup_rate_limits(&self, window: Duration) {
        let mut inner = self.inner.write().await;
        let now = Instant::now();
        inner
            .rate_limits
            .retain(|_, &mut last| now.duration_since(last) < window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};
    use std::time::Duration;

    #[tokio::test]
    async fn test_rate_limiting() {
        let temp_file =
            std::env::temp_dir().join(format!("test_fas_rl_{}.jsonl", uuid::Uuid::new_v4()));
        let store = Store::new(temp_file.clone());

        let ip = "192.168.1.100";
        let window = Duration::from_secs(2);

        // First request: OK
        assert!(store.check_rate_limit(ip, window).await.is_ok());

        // Second request within window: Rate limited
        let res = store.check_rate_limit(ip, window).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), 2); // retry after 2 seconds

        // Cleanup rate limits (simulate time elapsed)
        store.cleanup_rate_limits(Duration::from_secs(0)).await;

        // Should be OK again
        assert!(store.check_rate_limit(ip, window).await.is_ok());

        // Clean up temp file (it shouldn't even be created since we didn't save)
        if temp_file.exists() {
            let _ = std::fs::remove_file(temp_file);
        }
    }

    #[tokio::test]
    async fn test_store_save_load_purge() {
        let temp_file =
            std::env::temp_dir().join(format!("test_fas_data_{}.jsonl", uuid::Uuid::new_v4()));
        let store = Store::new(temp_file.clone());

        let user = User {
            sid: "test-sid-1".to_string(),
            domain: "test.com".to_string(),
            approved: false,
            created_at: Utc::now() - ChronoDuration::hours(2), // 2 hours old
            updated_at: Utc::now(),
            last_ip: "127.0.0.1".to_string(),
            last_seen: Utc::now(),
            user_agent: "test-ua".to_string(),
            request_count: 5,
        };

        let approved_user = User {
            sid: "test-sid-2".to_string(),
            domain: "test.com".to_string(),
            approved: true,
            created_at: Utc::now() - ChronoDuration::days(31), // 31 days old
            updated_at: Utc::now(),
            last_ip: "127.0.0.1".to_string(),
            last_seen: Utc::now(),
            user_agent: "test-ua".to_string(),
            request_count: 10,
        };

        // Add users
        {
            let mut inner = store.inner.write().await;
            inner.users.insert(user.sid.clone(), user.clone());
            inner
                .users
                .insert(approved_user.sid.clone(), approved_user.clone());
            inner.dirty = true;
        }

        // Save to file
        store.flush().await.expect("Failed to flush store");
        assert!(temp_file.exists());

        // Load from file in another store instance
        let store2 = Store::new(temp_file.clone());
        store2.load().await.expect("Failed to load store");

        {
            let inner2 = store2.inner.read().await;
            assert_eq!(inner2.users.len(), 2);
            assert_eq!(inner2.users.get("test-sid-1").unwrap(), &user);
            assert_eq!(inner2.users.get("test-sid-2").unwrap(), &approved_user);
        }

        // Purge:
        // unapproved_ttl = 1 hour, record_ttl = 30 days
        // test-sid-1: unapproved, 2 hours old -> should be purged
        // test-sid-2: approved, 31 days old -> should be purged
        let purged = store2
            .purge_old_records(
                Duration::from_secs(30 * 24 * 60 * 60),
                Duration::from_secs(60 * 60),
            )
            .await;
        assert_eq!(purged, 2);

        {
            let inner2 = store2.inner.read().await;
            assert_eq!(inner2.users.len(), 0);
        }

        // Clean up temp file
        let _ = std::fs::remove_file(temp_file);
    }
}
