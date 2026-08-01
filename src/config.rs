use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub data_file: PathBuf,
    pub acl_file: PathBuf,
    pub port: u16,
    pub cookie_max_age: i64,
    pub unapproved_ttl: Duration,
    pub purge_interval: Duration,
    pub rate_limit_window: Duration,
    pub save_interval: Duration,
    pub shared_secret: Option<String>,
    pub log_level: String,
    pub log_format: String,
}

impl Config {
    pub fn from_env() -> Self {
        let data_file = env::var("FAS_DATA_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data/fas.jsonl"));

        let acl_file = env::var("FAS_ACL_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data/acl.yaml"));

        let port = env::var("FAS_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let cookie_max_age = env::var("FAS_COOKIE_MAX_AGE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(90 * 24 * 60 * 60); // 90 days in seconds

        let unapproved_ttl_secs = env::var("FAS_UNAPPROVED_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60 * 60); // 1 hour

        let purge_interval_secs = env::var("FAS_PURGE_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60 * 60); // 1 hour

        let rate_limit_window_secs = env::var("FAS_RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5); // 5 seconds

        let save_interval_secs = env::var("FAS_SAVE_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30); // 30 seconds

        let shared_secret = env::var("FAS_SHARED_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let log_level = env::var("FAS_LOG_LEVEL")
            .or_else(|_| env::var("LOG_LEVEL"))
            .unwrap_or_else(|_| "info".to_string());

        let log_format = env::var("FAS_LOG_FORMAT")
            .or_else(|_| env::var("LOG_FORMAT"))
            .or_else(|_| {
                env::var("FAS_LOG_JSON").map(|v| {
                    if v == "1" || v.eq_ignore_ascii_case("true") {
                        "json".to_string()
                    } else {
                        "text".to_string()
                    }
                })
            })
            .unwrap_or_else(|_| "json".to_string());

        Self {
            data_file,
            acl_file,
            port,
            cookie_max_age,
            unapproved_ttl: Duration::from_secs(unapproved_ttl_secs),
            purge_interval: Duration::from_secs(purge_interval_secs),
            rate_limit_window: Duration::from_secs(rate_limit_window_secs),
            save_interval: Duration::from_secs(save_interval_secs),
            shared_secret,
            log_level,
            log_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_config_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();

        // 1. Defaults test
        env::remove_var("FAS_DATA_FILE");
        env::remove_var("FAS_ACL_FILE");
        env::remove_var("FAS_PORT");
        env::remove_var("FAS_COOKIE_MAX_AGE");
        env::remove_var("FAS_UNAPPROVED_TTL_SECS");
        env::remove_var("FAS_PURGE_INTERVAL_SECS");
        env::remove_var("FAS_RATE_LIMIT_WINDOW_SECS");
        env::remove_var("FAS_SAVE_INTERVAL_SECS");
        env::remove_var("FAS_SHARED_SECRET");
        env::remove_var("FAS_LOG_LEVEL");
        env::remove_var("LOG_LEVEL");
        env::remove_var("FAS_LOG_FORMAT");
        env::remove_var("LOG_FORMAT");
        env::remove_var("FAS_LOG_JSON");

        let config_def = Config::from_env();
        assert_eq!(config_def.data_file, PathBuf::from("/data/fas.jsonl"));
        assert_eq!(config_def.acl_file, PathBuf::from("/data/acl.yaml"));
        assert_eq!(config_def.port, 8080);
        assert_eq!(config_def.cookie_max_age, 90 * 24 * 60 * 60);
        assert_eq!(config_def.unapproved_ttl, Duration::from_secs(3600));
        assert_eq!(config_def.purge_interval, Duration::from_secs(3600));
        assert_eq!(config_def.rate_limit_window, Duration::from_secs(5));
        assert_eq!(config_def.save_interval, Duration::from_secs(30));
        assert_eq!(config_def.shared_secret, None);
        assert_eq!(config_def.log_level, "info");
        assert_eq!(config_def.log_format, "json");

        // 2. Custom values test
        env::set_var("FAS_DATA_FILE", "/custom/data.jsonl");
        env::set_var("FAS_ACL_FILE", "/custom/acl.yaml");
        env::set_var("FAS_PORT", "9090");
        env::set_var("FAS_COOKIE_MAX_AGE", "1000");
        env::set_var("FAS_UNAPPROVED_TTL_SECS", "300");
        env::set_var("FAS_PURGE_INTERVAL_SECS", "600");
        env::set_var("FAS_RATE_LIMIT_WINDOW_SECS", "10");
        env::set_var("FAS_SAVE_INTERVAL_SECS", "15");
        env::set_var("FAS_SHARED_SECRET", "secret_token_123");
        env::set_var("FAS_LOG_LEVEL", "debug");
        env::set_var("FAS_LOG_FORMAT", "text");

        let config_custom = Config::from_env();
        assert_eq!(config_custom.data_file, PathBuf::from("/custom/data.jsonl"));
        assert_eq!(config_custom.acl_file, PathBuf::from("/custom/acl.yaml"));
        assert_eq!(config_custom.port, 9090);
        assert_eq!(config_custom.cookie_max_age, 1000);
        assert_eq!(config_custom.unapproved_ttl, Duration::from_secs(300));
        assert_eq!(config_custom.purge_interval, Duration::from_secs(600));
        assert_eq!(config_custom.rate_limit_window, Duration::from_secs(10));
        assert_eq!(config_custom.save_interval, Duration::from_secs(15));
        assert_eq!(
            config_custom.shared_secret,
            Some("secret_token_123".to_string())
        );
        assert_eq!(config_custom.log_level, "debug");
        assert_eq!(config_custom.log_format, "text");

        // Clean up env vars
        env::remove_var("FAS_DATA_FILE");
        env::remove_var("FAS_ACL_FILE");
        env::remove_var("FAS_PORT");
        env::remove_var("FAS_COOKIE_MAX_AGE");
        env::remove_var("FAS_UNAPPROVED_TTL_SECS");
        env::remove_var("FAS_PURGE_INTERVAL_SECS");
        env::remove_var("FAS_RATE_LIMIT_WINDOW_SECS");
        env::remove_var("FAS_SAVE_INTERVAL_SECS");
        env::remove_var("FAS_SHARED_SECRET");
        env::remove_var("FAS_LOG_LEVEL");
        env::remove_var("LOG_LEVEL");
        env::remove_var("FAS_LOG_FORMAT");
        env::remove_var("LOG_FORMAT");
        env::remove_var("FAS_LOG_JSON");
    }
}
