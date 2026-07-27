use axum::{
    routing::{delete, get, post},
    Router, Server,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod acl;
mod config;
mod handlers;
mod i18n;
mod store;
mod templates;

use crate::config::Config;
use crate::handlers::AppState;
use crate::store::Store;

#[tokio::main]
async fn main() {
    // 1. Initialize tracing/logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fas=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 2. Load Configuration
    let config = Config::from_env();
    tracing::info!("Loaded config: {:?}", config);

    // 3. Initialize and Load Data Store & ACL Config
    let store = Store::new(config.data_file.clone(), config.acl_file.clone());
    if let Err(e) = store.load_acl().await {
        tracing::error!("Failed to load ACL config: {:?}", e);
    }
    if let Err(e) = store.load().await {
        tracing::error!("Failed to load data store: {:?}", e);
    }

    // 4. Spawn Background Tasks

    // A. Periodic and triggered save task (debounce writes)
    let store_for_save = store.clone();
    let save_interval = config.save_interval;
    let notify_save = store.notify_save.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = notify_save.notified() => {},
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
            }

            let should_save = {
                let inner = store_for_save.inner.read().await;
                inner.dirty && inner.last_save.elapsed() >= save_interval
            };

            if should_save {
                if let Err(e) = store_for_save.flush().await {
                    tracing::error!("Error saving store: {:?}", e);
                }
            }
        }
    });

    // B. Periodic records purge task (Cookie Max Age and Soft TTL)
    let store_for_purge = store.clone();
    let cookie_max_age = std::time::Duration::from_secs(config.cookie_max_age.max(0) as u64);
    let unapproved_ttl = config.unapproved_ttl;
    let purge_interval = config.purge_interval;
    tokio::spawn(async move {
        // Initial purge
        let initial_purged = store_for_purge
            .purge_old_records(cookie_max_age, unapproved_ttl)
            .await;
        if initial_purged > 0 {
            tracing::info!(
                "Startup: purged {} expired/unapproved records",
                initial_purged
            );
            if let Err(e) = store_for_purge.flush().await {
                tracing::error!("Startup: failed to save after purge: {:?}", e);
            }
        }

        let mut interval = tokio::time::interval(purge_interval);
        // tick immediately has already run/skipped
        interval.tick().await;
        loop {
            interval.tick().await;
            let purged = store_for_purge
                .purge_old_records(cookie_max_age, unapproved_ttl)
                .await;
            if purged > 0 {
                tracing::info!("Purged {} expired or unapproved records", purged);
                store_for_purge.mark_dirty(save_interval).await;
            }
        }
    });

    // C. Periodic rate limit map cleanup (every 30s)
    let store_for_rl = store.clone();
    let rl_window = config.rate_limit_window;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            store_for_rl.cleanup_rate_limits(rl_window).await;
        }
    });

    // 5. Setup Router and Web Server
    let state = AppState {
        store: store.clone(),
        config: config.clone(),
    };

    // Build optional token-check middleware closure
    let shared_secret = config.shared_secret.clone();
    let app = Router::new()
        .route("/_health", get(handlers::health_check))
        .route("/_auth", get(handlers::auth_handler))
        .route("/", get(handlers::admin_page_handler))
        .route("/api/stats", get(handlers::stats_handler))
        .route("/api/stats/reset", post(handlers::reset_stats_handler))
        .route("/api/users", get(handlers::list_users_handler))
        .route(
            "/api/users/:sid/rule",
            post(handlers::update_user_rule_handler),
        )
        .route(
            "/api/users/:sid/remark",
            post(handlers::update_remark_handler),
        )
        .route(
            "/api/users/:sid/extend",
            post(handlers::extend_user_ttl_handler),
        )
        .route("/api/users/:sid", delete(handlers::delete_user_handler))
        .route(
            "/api/config",
            get(handlers::get_config_handler).post(handlers::save_config_handler),
        )
        .route(
            "/api/config/validate",
            post(handlers::validate_config_handler),
        )
        .layer(axum::middleware::from_fn(move |req, next| {
            handlers::shared_secret_middleware(shared_secret.clone(), req, next)
        }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("FAS server starting on port {}", config.port);

    Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal(store))
        .await
        .unwrap();
}

async fn shutdown_signal(store: Store) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutting down... flushing database to disk");
    if let Err(e) = store.flush().await {
        tracing::error!("Error during final shutdown flush: {:?}", e);
    }
    tracing::info!("Shutdown complete");
}
