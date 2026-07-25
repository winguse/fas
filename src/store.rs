use crate::acl::{default_acl_config, parse_and_validate_yaml, AclConfig, CompiledAclConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{Notify, RwLock};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct User {
    pub sid: String,
    pub last_seen_domain: String,
    pub acl_rule: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub last_ip: String,
    pub last_seen: DateTime<Utc>,
    pub user_agent: String,
    pub request_count: u64,
    pub remark: String,
}

// Helper struct for deserializing User records (supporting legacy and updated formats)
#[derive(Deserialize)]
struct UserDeserializeHelper {
    sid: String,
    #[serde(alias = "domain")]
    domain: Option<String>,
    last_seen_domain: Option<String>,
    approved: Option<bool>,
    acl_rule: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expire_at: Option<DateTime<Utc>>,
    last_ip: String,
    last_seen: DateTime<Utc>,
    user_agent: String,
    request_count: u64,
    #[serde(default)]
    remark: String,
}

impl<'de> Deserialize<'de> for User {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = UserDeserializeHelper::deserialize(deserializer)?;
        let last_seen_domain = helper
            .last_seen_domain
            .or(helper.domain)
            .unwrap_or_default();

        let acl_rule = helper.acl_rule.unwrap_or_else(|| {
            if helper.approved == Some(true) {
                "allow_all".to_string()
            } else {
                "deny_all".to_string()
            }
        });

        let expire_at = helper.expire_at.unwrap_or_else(|| {
            let is_deny_rule = acl_rule == "deny_all" || acl_rule == "deny";
            let default_duration = if is_deny_rule {
                chrono::Duration::hours(1)
            } else {
                chrono::Duration::seconds(90 * 24 * 60 * 60)
            };
            helper.created_at + default_duration
        });

        Ok(User {
            sid: helper.sid,
            last_seen_domain,
            acl_rule,
            created_at: helper.created_at,
            updated_at: helper.updated_at,
            expire_at,
            last_ip: helper.last_ip,
            last_seen: helper.last_seen,
            user_agent: helper.user_agent,
            request_count: helper.request_count,
            remark: helper.remark,
        })
    }
}

pub struct StoreInner {
    pub users: HashMap<String, User>,
    pub rate_limits: HashMap<String, Instant>,
    pub dirty: bool,
    pub last_save: Instant,
    pub data_file: PathBuf,
    pub acl_file: PathBuf,
    pub acl_yaml: String,
    pub acl_config: AclConfig,
    pub compiled_acl: CompiledAclConfig,
    pub total_requests: u64,
}

#[derive(Clone)]
pub struct Store {
    pub inner: Arc<RwLock<StoreInner>>,
    pub notify_save: Arc<Notify>,
}

impl Store {
    pub fn new(data_file: PathBuf, acl_file: PathBuf) -> Self {
        let (default_cfg, default_compiled) = parse_and_validate_yaml("")
            .unwrap_or_else(|_| (default_acl_config(), CompiledAclConfig::default()));
        let default_yaml = serde_yaml::to_string(&default_cfg).unwrap_or_default();

        Self {
            inner: Arc::new(RwLock::new(StoreInner {
                users: HashMap::new(),
                rate_limits: HashMap::new(),
                dirty: false,
                last_save: Instant::now() - Duration::from_secs(3600),
                data_file,
                acl_file,
                acl_yaml: default_yaml,
                acl_config: default_cfg,
                compiled_acl: default_compiled,
                total_requests: 0,
            })),
            notify_save: Arc::new(Notify::new()),
        }
    }

    /// Load ACL YAML file from disk
    pub async fn load_acl(&self) -> std::io::Result<()> {
        let acl_file = {
            let inner = self.inner.read().await;
            inner.acl_file.clone()
        };

        if !acl_file.exists() {
            tracing::info!(
                "No existing ACL file at {:?} — using default ACL config",
                acl_file
            );
            let (cfg, compiled) = parse_and_validate_yaml("").unwrap();
            let yaml_str = serde_yaml::to_string(&cfg).unwrap();
            let mut inner = self.inner.write().await;
            inner.acl_yaml = yaml_str;
            inner.acl_config = cfg;
            inner.compiled_acl = compiled;
            return Ok(());
        }

        let content = fs::read_to_string(&acl_file).await?;
        match parse_and_validate_yaml(&content) {
            Ok((cfg, compiled)) => {
                let mut inner = self.inner.write().await;
                inner.acl_yaml = content;
                inner.acl_config = cfg;
                inner.compiled_acl = compiled;
                tracing::info!("Loaded ACL config from {:?}", acl_file);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to parse ACL config from {:?}: {}", acl_file, e);
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
        }
    }

    /// Update ACL YAML config in memory and save to disk
    pub async fn update_acl(&self, yaml_str: &str) -> Result<(), String> {
        let (cfg, compiled) = parse_and_validate_yaml(yaml_str)?;

        let acl_file = {
            let mut inner = self.inner.write().await;
            inner.acl_yaml = yaml_str.to_string();
            inner.acl_config = cfg;
            inner.compiled_acl = compiled;
            inner.acl_file.clone()
        };

        if let Some(parent) = acl_file.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        fs::write(&acl_file, yaml_str)
            .await
            .map_err(|e| format!("Failed to write ACL file {:?}: {}", acl_file, e))?;

        tracing::info!("Saved updated ACL config to {:?}", acl_file);
        Ok(())
    }

    /// Load store users & stats from file
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
        let mut has_meta_stats = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains("\"meta\":\"stats\"") {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(total) = val.get("total_requests").and_then(|v| v.as_u64()) {
                        inner.total_requests = total;
                        has_meta_stats = true;
                    }
                }
                continue;
            }
            if let Ok(u) = serde_json::from_str::<User>(trimmed) {
                inner.users.insert(u.sid.clone(), u);
                count += 1;
            }
        }
        if !has_meta_stats {
            inner.total_requests = inner.users.values().map(|u| u.request_count).sum();
        }
        tracing::info!("Loaded {} records from {:?}", count, data_file);
        Ok(())
    }

    /// Save dirty store to file
    pub async fn flush(&self) -> std::io::Result<()> {
        let (users, total_requests, data_file) = {
            let mut inner = self.inner.write().await;
            if !inner.dirty {
                return Ok(());
            }
            inner.dirty = false;
            inner.last_save = Instant::now();
            (
                inner.users.values().cloned().collect::<Vec<User>>(),
                inner.total_requests,
                inner.data_file.clone(),
            )
        };

        if let Some(parent) = data_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut content = String::new();

        // Write the stats metadata line first
        if let Ok(line) = serde_json::to_string(&serde_json::json!({
            "meta": "stats",
            "total_requests": total_requests
        })) {
            content.push_str(&line);
            content.push('\n');
        }

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

    /// Purge expired records (based on expire_at or cookie max age / unapproved TTL)
    pub async fn purge_old_records(
        &self,
        cookie_max_age: Duration,
        unapproved_ttl: Duration,
    ) -> usize {
        let mut to_delete = Vec::new();
        let now_utc = Utc::now();

        {
            let inner = self.inner.read().await;
            for (sid, user) in inner.users.iter() {
                let elapsed_created = now_utc.signed_duration_since(user.created_at);

                let max_age_chrono = chrono::Duration::from_std(cookie_max_age)
                    .unwrap_or_else(|_| chrono::Duration::max_value());
                let unapp_ttl_chrono = chrono::Duration::from_std(unapproved_ttl)
                    .unwrap_or_else(|_| chrono::Duration::max_value());

                let is_deny_rule = user.acl_rule == "deny_all" || user.acl_rule == "deny";

                if now_utc >= user.expire_at
                    || elapsed_created >= max_age_chrono
                    || (is_deny_rule && elapsed_created >= unapp_ttl_chrono)
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
        let temp_dir = std::env::temp_dir();
        let temp_data = temp_dir.join(format!("test_fas_rl_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_acl = temp_dir.join(format!("test_fas_rl_{}.yaml", uuid::Uuid::new_v4()));
        let store = Store::new(temp_data.clone(), temp_acl.clone());

        let ip = "192.168.1.100";
        let window = Duration::from_secs(2);

        // First request: OK
        assert!(store.check_rate_limit(ip, window).await.is_ok());

        // Second request within window: Rate limited
        let res = store.check_rate_limit(ip, window).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), 2);

        // Cleanup rate limits (simulate time elapsed)
        store.cleanup_rate_limits(Duration::from_secs(0)).await;

        // Should be OK again
        assert!(store.check_rate_limit(ip, window).await.is_ok());

        if temp_data.exists() {
            let _ = std::fs::remove_file(temp_data);
        }
        if temp_acl.exists() {
            let _ = std::fs::remove_file(temp_acl);
        }
    }

    #[tokio::test]
    async fn test_store_save_load_purge() {
        let temp_dir = std::env::temp_dir();
        let temp_data = temp_dir.join(format!("test_fas_data_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_acl = temp_dir.join(format!("test_fas_data_{}.yaml", uuid::Uuid::new_v4()));
        let store = Store::new(temp_data.clone(), temp_acl.clone());

        let now = Utc::now();
        let user = User {
            sid: "test-sid-1".to_string(),
            last_seen_domain: "test.com".to_string(),
            acl_rule: "deny_all".to_string(),
            created_at: now - ChronoDuration::hours(2),
            updated_at: now,
            expire_at: now - ChronoDuration::hours(1), // already expired
            last_ip: "127.0.0.1".to_string(),
            last_seen: now,
            user_agent: "test-ua".to_string(),
            request_count: 5,
            remark: String::new(),
        };

        let approved_user = User {
            sid: "test-sid-2".to_string(),
            last_seen_domain: "test.com".to_string(),
            acl_rule: "allow_all".to_string(),
            created_at: now - ChronoDuration::days(31),
            updated_at: now,
            expire_at: now - ChronoDuration::days(1), // already expired
            last_ip: "127.0.0.1".to_string(),
            last_seen: now,
            user_agent: "test-ua".to_string(),
            request_count: 10,
            remark: String::new(),
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
        assert!(temp_data.exists());

        // Load from file in another store instance
        let store2 = Store::new(temp_data.clone(), temp_acl.clone());
        store2.load().await.expect("Failed to load store");

        {
            let inner2 = store2.inner.read().await;
            assert_eq!(inner2.users.len(), 2);
            assert_eq!(inner2.users.get("test-sid-1").unwrap(), &user);
            assert_eq!(inner2.users.get("test-sid-2").unwrap(), &approved_user);
        }

        // Purge
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

        let _ = std::fs::remove_file(temp_data);
        let _ = std::fs::remove_file(temp_acl);
    }

    #[test]
    fn test_legacy_user_deserialization_computes_expire_at() {
        let legacy_json_approved = r#"{
            "sid": "legacy-1",
            "domain": "olddomain.com",
            "approved": true,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_ip": "1.1.1.1",
            "last_seen": "2026-01-01T00:00:00Z",
            "user_agent": "Mozilla",
            "request_count": 5,
            "remark": "friend"
        }"#;

        let u: User = serde_json::from_str(legacy_json_approved)
            .expect("Failed to deserialize legacy approved user");
        assert_eq!(u.sid, "legacy-1");
        assert_eq!(u.last_seen_domain, "olddomain.com");
        assert_eq!(u.acl_rule, "allow_all");
        // Computed expire_at for allow_all should be created_at + 90 days (cookie max age)
        assert_eq!(u.expire_at, u.created_at + ChronoDuration::days(90));

        let legacy_json_unapproved = r#"{
            "sid": "legacy-2",
            "domain": "olddomain2.com",
            "approved": false,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "last_ip": "1.1.1.1",
            "last_seen": "2026-01-01T00:00:00Z",
            "user_agent": "Mozilla",
            "request_count": 1
        }"#;

        let u2: User = serde_json::from_str(legacy_json_unapproved)
            .expect("Failed to deserialize legacy unapproved user");
        assert_eq!(u2.sid, "legacy-2");
        assert_eq!(u2.last_seen_domain, "olddomain2.com");
        assert_eq!(u2.acl_rule, "deny_all");
        // Computed expire_at for deny_all should be created_at + 1 hour
        assert_eq!(u2.expire_at, u2.created_at + ChronoDuration::hours(1));
    }

    #[tokio::test]
    async fn test_purge_by_expire_at() {
        let temp_dir = std::env::temp_dir();
        let temp_data = temp_dir.join(format!("test_fas_expire_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_acl = temp_dir.join(format!("test_fas_expire_{}.yaml", uuid::Uuid::new_v4()));
        let store = Store::new(temp_data.clone(), temp_acl.clone());

        let now = Utc::now();
        let active_user = User {
            sid: "active-1".to_string(),
            last_seen_domain: "example.com".to_string(),
            acl_rule: "allow_all".to_string(),
            created_at: now,
            updated_at: now,
            expire_at: now + ChronoDuration::hours(5),
            last_ip: "127.0.0.1".to_string(),
            last_seen: now,
            user_agent: "test".to_string(),
            request_count: 1,
            remark: String::new(),
        };

        let expired_user = User {
            sid: "expired-1".to_string(),
            last_seen_domain: "example.com".to_string(),
            acl_rule: "allow_all".to_string(),
            created_at: now - ChronoDuration::hours(10),
            updated_at: now,
            expire_at: now - ChronoDuration::seconds(10), // expired 10s ago
            last_ip: "127.0.0.1".to_string(),
            last_seen: now,
            user_agent: "test".to_string(),
            request_count: 1,
            remark: String::new(),
        };

        {
            let mut inner = store.inner.write().await;
            inner
                .users
                .insert(active_user.sid.clone(), active_user.clone());
            inner
                .users
                .insert(expired_user.sid.clone(), expired_user.clone());
        }

        let purged = store
            .purge_old_records(
                Duration::from_secs(30 * 24 * 60 * 60),
                Duration::from_secs(60 * 60),
            )
            .await;

        assert_eq!(purged, 1);

        {
            let inner = store.inner.read().await;
            assert_eq!(inner.users.len(), 1);
            assert!(inner.users.contains_key("active-1"));
            assert!(!inner.users.contains_key("expired-1"));
        }

        let _ = std::fs::remove_file(temp_data);
        let _ = std::fs::remove_file(temp_acl);
    }
}
