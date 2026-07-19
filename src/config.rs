use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub data_file: PathBuf,
    pub port: u16,
    pub cookie_max_age: i64,
    pub record_ttl: Duration,
    pub unapproved_ttl: Duration,
    pub purge_interval: Duration,
    pub rate_limit_window: Duration,
    pub save_interval: Duration,
}

impl Config {
    pub fn from_env() -> Self {
        let data_file = env::var("FAS_DATA_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data/fas.jsonl"));

        let port = env::var("FAS_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let cookie_max_age = env::var("FAS_COOKIE_MAX_AGE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(90 * 24 * 60 * 60); // 90 days in seconds

        let record_ttl_secs = env::var("FAS_RECORD_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30 * 24 * 60 * 60); // 30 days

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

        Self {
            data_file,
            port,
            cookie_max_age,
            record_ttl: Duration::from_secs(record_ttl_secs),
            unapproved_ttl: Duration::from_secs(unapproved_ttl_secs),
            purge_interval: Duration::from_secs(purge_interval_secs),
            rate_limit_window: Duration::from_secs(rate_limit_window_secs),
            save_interval: Duration::from_secs(save_interval_secs),
        }
    }
}
