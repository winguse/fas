use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Json, Response};
use chrono::Utc;
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

use crate::config::Config;
use crate::i18n::{detect_locale, Locale};
use crate::store::Store;
use crate::templates::escape_html;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub config: Config,
}

#[derive(Deserialize)]
pub struct AuthQuery {
    pub domain: Option<String>,
}

/// Helper: Extract the real client IP from forwarded headers.
pub fn real_client_ip(headers: &HeaderMap) -> String {
    // RFC 7239 Forwarded header
    if let Some(fwd) = headers.get("Forwarded").and_then(|v| v.to_str().ok()) {
        static RE_FWD: OnceLock<Regex> = OnceLock::new();
        let re = RE_FWD.get_or_init(|| Regex::new(r#"(?i)for="?([^";,\s]+)"?"#).unwrap());
        if let Some(caps) = re.captures(fwd) {
            if let Some(m) = caps.get(1) {
                return m.as_str().to_string();
            }
        }
    }
    // X-Real-Ip
    if let Some(real_ip) = headers.get("X-Real-Ip").and_then(|v| v.to_str().ok()) {
        return real_ip.to_string();
    }
    // X-Forwarded-For
    if let Some(xff) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
        if let Some(first_ip) = xff.split(',').next() {
            return first_ip.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Helper: Detect locale from Accept-Language
pub fn get_locale(headers: &HeaderMap) -> Locale {
    let accept = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    detect_locale(accept)
}

/// Helper: Extract the visitor session ID from cookies
pub fn extract_sid(headers: &HeaderMap) -> String {
    if let Some(cookie_val) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        static RE_COOKIE: OnceLock<Regex> = OnceLock::new();
        let re = RE_COOKIE.get_or_init(|| Regex::new(r"fas_sid=([^;]+)").unwrap());
        if let Some(caps) = re.captures(cookie_val) {
            if let Some(m) = caps.get(1) {
                return m.as_str().to_string();
            }
        }
    }
    String::new()
}

fn make_set_cookie(sid: &str, cookie_domain: Option<&str>, max_age: i64) -> String {
    if let Some(cd) = cookie_domain {
        format!(
            "fas_sid={}; Path=/; HttpOnly; SameSite=Lax; Domain={}; Max-Age={}",
            sid, cd, max_age
        )
    } else {
        format!(
            "fas_sid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            sid, max_age
        )
    }
}

/// Axum 0.6 Middleware to check X-Shared-Secret header for all endpoints except `/_auth`
pub async fn shared_secret_middleware<B>(
    shared_secret: Option<String>,
    req: axum::http::Request<B>,
    next: axum::middleware::Next<B>,
) -> Response
where
    B: Send + 'static,
{
    // Always allow /_auth through (visitor auth endpoint)
    if req.uri().path() == "/_auth" || req.uri().path() == "/_health" || req.uri().path() == "/_ready" {
        return next.run(req).await;
    }

    if let Some(expected_token) = &shared_secret {
        let header_token = req
            .headers()
            .get("x-shared-secret")
            .and_then(|v| v.to_str().ok());

        if header_token != Some(expected_token.as_str()) {
            use axum::body::{boxed, Full};
            let body = serde_json::json!({
                "ok": false,
                "error": "Unauthorized: missing or invalid X-Shared-Secret header"
            })
            .to_string();
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(boxed(Full::from(body)))
                .unwrap();
        }
    }

    next.run(req).await
}

fn compute_user_expire_at(
    compiled_acl: &crate::acl::CompiledAclConfig,
    rule_name: &str,
    domain: &str,
    config: &Config,
) -> chrono::DateTime<Utc> {
    let is_allowed = compiled_acl.evaluate(rule_name, "GET", domain, "/");
    let duration = if is_allowed {
        chrono::Duration::seconds(config.cookie_max_age)
    } else {
        chrono::Duration::from_std(config.unapproved_ttl)
            .unwrap_or_else(|_| chrono::Duration::hours(1))
    };
    Utc::now() + duration
}

/// GET /_health
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

pub async fn ready_check(State(state): State<AppState>) -> impl IntoResponse {
    let is_ready = state.store.inner.read().await.is_ready;
    if is_ready {
        (StatusCode::OK, "OK")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Not Ready")
    }
}

/// GET /_auth
pub async fn auth_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> impl IntoResponse {
    let locale = get_locale(&headers);
    let sid = extract_sid(&headers);
    let client_ip = real_client_ip(&headers);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();

    let domain = query
        .domain
        .or_else(|| {
            headers
                .get("X-Forwarded-Host")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            headers
                .get("Host")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    let method = headers
        .get("X-Forwarded-Method")
        .or_else(|| headers.get("X-Original-Method"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("GET")
        .to_string();

    let path = headers
        .get("X-Forwarded-Uri")
        .or_else(|| headers.get("X-Original-URI"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/")
        .to_string();

    let s = crate::i18n::t(locale);

    // Increment dedicated total requests counter
    {
        let mut inner = state.store.inner.write().await;
        inner.total_requests = inner.total_requests.saturating_add(1);
        inner.dirty = true;
    }
    state.store.mark_dirty(state.config.save_interval).await;

    let mut user_found = None;
    if !sid.is_empty() {
        let mut inner = state.store.inner.write().await;
        if let Some(user) = inner.users.get_mut(&sid) {
            user.last_seen_domain = domain.clone();
            user.last_ip = client_ip.clone();
            user.last_seen = Utc::now();
            user.user_agent = user_agent.clone();
            user.request_count = user.request_count.saturating_add(1);
            user.updated_at = Utc::now();
            user_found = Some(user.clone());
        }
    }

    if let Some(user) = user_found {
        let (is_allowed, cookie_domain_opt) = {
            let inner = state.store.inner.read().await;
            let allowed = inner
                .compiled_acl
                .evaluate(&user.acl_rule, &method, &domain, &path);
            let cdom = inner.compiled_acl.resolve_cookie_domain(&domain);
            (allowed, cdom)
        };

        if is_allowed {
            state.store.mark_dirty(state.config.save_interval).await;
            let remaining_ttl = (user.expire_at - Utc::now()).num_seconds().max(1);
            let set_cookie_hdr =
                make_set_cookie(&user.sid, cookie_domain_opt.as_deref(), remaining_ttl);

            return Response::builder()
                .status(StatusCode::OK)
                .header(header::SET_COOKIE, set_cookie_hdr)
                .body(axum::body::Body::from("Authorized"))
                .unwrap();
        }

        // Access denied by ACL — check rate limit before serving visitor page
        if let Err(retry_after) = state
            .store
            .check_rate_limit(&client_ip, state.config.rate_limit_window)
            .await
        {
            let html = crate::templates::rate_limit_page(locale, retry_after, &client_ip);
            return Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .header(header::RETRY_AFTER, retry_after.to_string())
                .body(axum::body::Body::from(html))
                .unwrap();
        }

        // Show pending/denied visitor page
        let short_sid = if user.sid.len() >= 6 {
            &user.sid[0..6]
        } else {
            &user.sid
        };
        let body_html = format!(
            r#"<h1>{}</h1>
<p>{}</p>
<div class="id-box"><span id="visitorId">{}</span><button class="copy-btn" onclick="copyId()">{}</button></div>
<p><span class="badge badge-warn">⏳ {}</span></p>
<p id="checkStatus" style="font-size: 0.85rem; color: #94a3b8; margin: 1rem 0; min-height: 1.2rem;"></p>
<p>{}</p>"#,
            s.visitor_wait_heading,
            s.visitor_wait_body,
            escape_html(short_sid),
            s.copy_btn,
            s.badge_pending,
            s.visitor_wait_footer
        );
        let html = crate::templates::visitor_page(locale, s.visitor_wait_title, &body_html);
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(axum::body::Body::from(html))
            .unwrap();
    }

    // No valid cookie — check rate limit before creating a new visitor record
    if let Err(retry_after) = state
        .store
        .check_rate_limit(&client_ip, state.config.rate_limit_window)
        .await
    {
        let html = crate::templates::rate_limit_page(locale, retry_after, &client_ip);
        return Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::RETRY_AFTER, retry_after.to_string())
            .body(axum::body::Body::from(html))
            .unwrap();
    }

    // Generate a new visitor with default "deny_all" rule and computed expire_at
    let new_sid = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now();
    let unapp_duration = chrono::Duration::from_std(state.config.unapproved_ttl)
        .unwrap_or_else(|_| chrono::Duration::hours(1));
    let expire_at = created_at + unapp_duration;

    let new_user = crate::store::User {
        sid: new_sid.clone(),
        last_seen_domain: domain.clone(),
        acl_rule: String::new(),
        created_at,
        updated_at: created_at,
        expire_at,
        last_ip: client_ip.clone(),
        last_seen: created_at,
        user_agent: user_agent.clone(),
        request_count: 1,
        remark: String::new(),
    };

    let cookie_domain_opt = {
        let mut inner = state.store.inner.write().await;
        let cdom = inner.compiled_acl.resolve_cookie_domain(&domain);
        inner.users.insert(new_sid.clone(), new_user.clone());
        cdom
    };
    state.store.mark_dirty(state.config.save_interval).await;
    tracing::info!("New visitor: {} on {} from {}", new_sid, domain, client_ip);

    let short_new_sid = if new_sid.len() >= 6 {
        &new_sid[0..6]
    } else {
        &new_sid
    };
    let body_html = format!(
        r#"<h1>{}</h1>
<p>{}</p>
<div class="id-box"><span id="visitorId">{}</span><button class="copy-btn" onclick="copyId()">{}</button></div>
<p><span class="badge badge-warn">⏳ {}</span></p>
<p id="checkStatus" style="font-size: 0.85rem; color: #94a3b8; margin: 1rem 0; min-height: 1.2rem;"></p>
<p>{}</p>"#,
        s.visitor_new_heading,
        s.visitor_new_body,
        escape_html(short_new_sid),
        s.copy_btn,
        s.badge_pending,
        s.visitor_new_footer
    );
    let html = crate::templates::visitor_page(locale, s.visitor_new_title, &body_html);

    let remaining_ttl = (new_user.expire_at - Utc::now()).num_seconds().max(1);
    let set_cookie_hdr = make_set_cookie(&new_sid, cookie_domain_opt.as_deref(), remaining_ttl);

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::SET_COOKIE, set_cookie_hdr)
        .body(axum::body::Body::from(html))
        .unwrap()
}

/// GET /api/stats
pub async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.store.inner.read().await;
    let total_users = inner.users.len();
    let total_reqs = inner.total_requests;

    Json(serde_json::json!({
        "ok": true,
        "totalUsers": total_users,
        "totalReqs": total_reqs
    }))
}

/// POST /api/stats/reset
pub async fn reset_stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut inner = state.store.inner.write().await;
    inner.total_requests = 0;
    inner.dirty = true;
    drop(inner);
    state.store.mark_dirty(state.config.save_interval).await;
    Json(serde_json::json!({
        "ok": true,
        "totalReqs": 0
    }))
}

/// GET /api/users
pub async fn list_users_handler(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.store.inner.read().await;
    let mut users: Vec<crate::store::User> = inner.users.values().cloned().collect();
    users.sort_by_key(|u| std::cmp::Reverse(u.created_at));

    let mut users_json = Vec::new();
    for u in users {
        let short_sid = if u.sid.len() >= 6 {
            u.sid[0..6].to_string()
        } else {
            u.sid.clone()
        };
        if let Ok(mut val) = serde_json::to_value(&u) {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("sid".to_string(), serde_json::Value::String(short_sid));
            }
            users_json.push(val);
        }
    }

    Json(serde_json::json!({
        "ok": true,
        "users": users_json
    }))
}

#[derive(Deserialize)]
pub struct RuleRequest {
    pub acl_rule: String,
}

/// POST /api/users/:sid/rule
pub async fn update_user_rule_handler(
    State(state): State<AppState>,
    Path(sid): Path<String>,
    Json(payload): Json<RuleRequest>,
) -> impl IntoResponse {
    let mut inner = state.store.inner.write().await;
    let target_sid = inner
        .users
        .keys()
        .find(|k| *k == &sid || (sid.len() >= 6 && k.starts_with(&sid)))
        .cloned();
    let compiled_acl = inner.compiled_acl.clone();
    if let Some(full_sid) = target_sid {
        if let Some(user) = inner.users.get_mut(&full_sid) {
            let domain = user.last_seen_domain.clone();
            user.acl_rule = payload.acl_rule.clone();
            user.updated_at = Utc::now();
            user.expire_at =
                compute_user_expire_at(&compiled_acl, &payload.acl_rule, &domain, &state.config);
            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            return (StatusCode::OK, Json(serde_json::json!({ "ok": true })));
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "ok": false, "error": "User not found" })),
    )
}

/// DELETE /api/users/:sid
pub async fn delete_user_handler(
    State(state): State<AppState>,
    Path(sid): Path<String>,
) -> impl IntoResponse {
    let mut inner = state.store.inner.write().await;
    let target_sid = inner
        .users
        .keys()
        .find(|k| *k == &sid || (sid.len() >= 6 && k.starts_with(&sid)))
        .cloned();
    if let Some(full_sid) = target_sid {
        if inner.users.remove(&full_sid).is_some() {
            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            tracing::info!("Deleted: {}", full_sid);
            let short_deleted = if full_sid.len() >= 6 {
                full_sid[0..6].to_string()
            } else {
                full_sid.clone()
            };
            return (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "deleted": short_deleted })),
            );
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "ok": false, "error": "User not found" })),
    )
}

/// GET /api/config
pub async fn get_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.store.inner.read().await;
    let yaml = inner.acl_yaml.clone();
    let mut rules: Vec<String> = inner.acl_config.acl_rules.keys().cloned().collect();
    rules.sort();

    Json(serde_json::json!({
        "ok": true,
        "yaml": yaml,
        "rules": rules
    }))
}

#[derive(Deserialize)]
pub struct ConfigSaveRequest {
    pub yaml: String,
}

/// POST /api/config
pub async fn save_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<ConfigSaveRequest>,
) -> impl IntoResponse {
    match state.store.update_acl(&payload.yaml).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": err })),
        ),
    }
}

/// POST /api/config/validate
pub async fn validate_config_handler(Json(payload): Json<ConfigSaveRequest>) -> impl IntoResponse {
    match crate::acl::parse_and_validate_yaml(&payload.yaml) {
        Ok((cfg, _, _)) => {
            let mut rules: Vec<String> = cfg.acl_rules.keys().cloned().collect();
            rules.sort();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "rules": rules })),
            )
        }
        Err(err) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": false, "error": err })),
        ),
    }
}

/// GET / (Admin page)
pub async fn admin_page_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let locale = get_locale(&headers);
    let (users, total_users, total_reqs, acl_yaml, acl_rules) = {
        let inner = state.store.inner.read().await;
        let mut users: Vec<crate::store::User> = inner.users.values().cloned().collect();
        users.sort_by_key(|u| std::cmp::Reverse(u.created_at));
        let total_users = inner.users.len();
        let total_reqs = inner.total_requests;
        let acl_yaml = inner.acl_yaml.clone();
        let mut acl_rules: Vec<String> = inner.acl_config.acl_rules.keys().cloned().collect();
        acl_rules.sort();
        (users, total_users, total_reqs, acl_yaml, acl_rules)
    };

    let table_rows = crate::templates::admin_table_rows(locale, &users, &acl_rules);
    let html = crate::templates::admin_page(
        locale,
        &table_rows,
        total_users,
        total_reqs,
        &acl_yaml,
        &acl_rules,
    );

    Html(html)
}

#[derive(Deserialize)]
pub struct RemarkRequest {
    pub remark: String,
}

/// POST /api/users/:sid/remark
pub async fn update_remark_handler(
    State(state): State<AppState>,
    Path(sid): Path<String>,
    Json(payload): Json<RemarkRequest>,
) -> impl IntoResponse {
    let mut inner = state.store.inner.write().await;
    let target_sid = inner
        .users
        .keys()
        .find(|k| *k == &sid || (sid.len() >= 6 && k.starts_with(&sid)))
        .cloned();
    if let Some(full_sid) = target_sid {
        if let Some(user) = inner.users.get_mut(&full_sid) {
            user.remark = payload.remark;
            user.updated_at = Utc::now();
            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            return (StatusCode::OK, Json(serde_json::json!({ "ok": true })));
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "ok": false, "error": "User not found" })),
    )
}

/// POST /api/users/:sid/extend
pub async fn extend_user_ttl_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sid): Path<String>,
) -> impl IntoResponse {
    let current_sid = extract_sid(&headers);
    let mut inner = state.store.inner.write().await;
    let target_sid = inner
        .users
        .keys()
        .find(|k| *k == &sid || (sid.len() >= 6 && k.starts_with(&sid)))
        .cloned();
    let compiled_acl = inner.compiled_acl.clone();
    if let Some(full_sid) = target_sid {
        if let Some(user) = inner.users.get_mut(&full_sid) {
            let is_allowed =
                compiled_acl.evaluate(&user.acl_rule, "GET", &user.last_seen_domain, "/");
            let default_ttl = if is_allowed {
                chrono::Duration::seconds(state.config.cookie_max_age)
            } else {
                chrono::Duration::from_std(state.config.unapproved_ttl)
                    .unwrap_or_else(|_| chrono::Duration::hours(1))
            };

            let now = Utc::now();
            let base_time = std::cmp::max(user.expire_at, now);
            user.expire_at = base_time + default_ttl;
            user.updated_at = now;
            let updated_expire_at = user.expire_at;
            let domain = user.last_seen_domain.clone();
            let cookie_domain_opt = inner.compiled_acl.resolve_cookie_domain(&domain);

            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            tracing::info!(
                "Extended TTL for user {}: new expire_at={}",
                full_sid,
                updated_expire_at
            );

            let remaining_ttl = (updated_expire_at - now).num_seconds().max(1);

            let mut res = (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "expire_at": updated_expire_at
                })),
            )
                .into_response();

            if !current_sid.is_empty()
                && (current_sid == full_sid || full_sid.starts_with(&current_sid))
            {
                let set_cookie_hdr =
                    make_set_cookie(&full_sid, cookie_domain_opt.as_deref(), remaining_ttl);
                if let Ok(hdr_val) = axum::http::HeaderValue::from_str(&set_cookie_hdr) {
                    res.headers_mut().insert(header::SET_COOKIE, hdr_val);
                }
            }

            return res;
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "ok": false, "error": "User not found" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_real_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-Ip", HeaderValue::from_static("1.2.3.4"));
        assert_eq!(real_client_ip(&headers), "1.2.3.4");

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("5.6.7.8, 1.2.3.4"),
        );
        assert_eq!(real_client_ip(&headers), "5.6.7.8");

        let mut headers = HeaderMap::new();
        headers.insert(
            "Forwarded",
            HeaderValue::from_static("for=9.10.11.12;proto=https"),
        );
        assert_eq!(real_client_ip(&headers), "9.10.11.12");

        let headers = HeaderMap::new();
        assert_eq!(real_client_ip(&headers), "unknown");
    }

    #[test]
    fn test_extract_sid() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("fas_sid=abcdef12345; other_cookie=xyz"),
        );
        assert_eq!(extract_sid(&headers), "abcdef12345");

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("other_cookie=xyz; fas_sid=abcdef12345"),
        );
        assert_eq!(extract_sid(&headers), "abcdef12345");

        let headers = HeaderMap::new();
        assert_eq!(extract_sid(&headers), "");
    }

    #[tokio::test]
    async fn test_auth_handler_new_visitor_and_allow() {
        let temp_dir = std::env::temp_dir();
        let temp_data = temp_dir.join(format!("test_fas_h1_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_acl = temp_dir.join(format!("test_fas_h1_{}.yaml", uuid::Uuid::new_v4()));
        let store = Store::new(temp_data.clone(), temp_acl.clone());
        let config = Config::from_env();

        let state = AppState {
            store: store.clone(),
            config: config.clone(),
        };

        // 1. Initial auth request (no cookie) -> 401 Unauthorized + Set-Cookie header
        let mut headers = HeaderMap::new();
        headers.insert("Host", HeaderValue::from_static("example.com"));
        headers.insert("X-Real-Ip", HeaderValue::from_static("10.0.0.1"));
        let query = AuthQuery { domain: None };

        let resp = auth_handler(State(state.clone()), headers, Query(query))
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let set_cookie = resp
            .headers()
            .get(header::SET_COOKIE)
            .expect("Set-Cookie header missing");
        let cookie_str = set_cookie.to_str().unwrap();
        assert!(cookie_str.contains("fas_sid="));

        // Extract SID from Set-Cookie header
        let sid = cookie_str
            .split(';')
            .next()
            .unwrap()
            .trim_start_matches("fas_sid=");

        // 2. Second auth request with new SID (deny_all by default) -> 401 Unauthorized
        let mut headers2 = HeaderMap::new();
        headers2.insert("Host", HeaderValue::from_static("example.com"));
        headers2.insert("X-Real-Ip", HeaderValue::from_static("10.0.0.2"));
        headers2.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("fas_sid={}", sid)).unwrap(),
        );
        let query2 = AuthQuery { domain: None };

        let resp2 = auth_handler(State(state.clone()), headers2, Query(query2))
            .await
            .into_response();
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

        // 3. Approve user via update_user_rule_handler ("allow_all")
        let rule_resp = update_user_rule_handler(
            State(state.clone()),
            Path(sid.to_string()),
            Json(RuleRequest {
                acl_rule: "✅ allow_all".to_string(),
            }),
        )
        .await
        .into_response();
        assert_eq!(rule_resp.status(), StatusCode::OK);

        // 4. Third auth request with approved SID -> 200 OK
        let mut headers3 = HeaderMap::new();
        headers3.insert("Host", HeaderValue::from_static("example.com"));
        headers3.insert("X-Real-Ip", HeaderValue::from_static("10.0.0.3"));
        headers3.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("fas_sid={}", sid)).unwrap(),
        );
        let query3 = AuthQuery { domain: None };

        let resp3 = auth_handler(State(state.clone()), headers3, Query(query3))
            .await
            .into_response();
        assert_eq!(resp3.status(), StatusCode::OK);

        if temp_data.exists() {
            let _ = std::fs::remove_file(temp_data);
        }
        if temp_acl.exists() {
            let _ = std::fs::remove_file(temp_acl);
        }
    }

    #[tokio::test]
    async fn test_api_user_management_endpoints() {
        let temp_dir = std::env::temp_dir();
        let temp_data = temp_dir.join(format!("test_fas_h2_{}.jsonl", uuid::Uuid::new_v4()));
        let temp_acl = temp_dir.join(format!("test_fas_h2_{}.yaml", uuid::Uuid::new_v4()));
        let store = Store::new(temp_data.clone(), temp_acl.clone());
        let config = Config::from_env();

        let state = AppState {
            store: store.clone(),
            config: config.clone(),
        };

        // Create a user in store
        let now = Utc::now();
        let test_user = crate::store::User {
            sid: "12345678-abcd-efgh-ijkl-1234567890ab".to_string(),
            last_seen_domain: "test.org".to_string(),
            acl_rule: "deny_all".to_string(),
            created_at: now,
            updated_at: now,
            expire_at: now + chrono::Duration::hours(1),
            last_ip: "10.0.0.1".to_string(),
            last_seen: now,
            user_agent: "TestAgent".to_string(),
            request_count: 1,
            remark: "".to_string(),
        };

        {
            let mut inner = store.inner.write().await;
            inner.users.insert(test_user.sid.clone(), test_user.clone());
        }

        // Test update_user_rule_handler
        let rule_resp = update_user_rule_handler(
            State(state.clone()),
            Path("123456".to_string()),
            Json(RuleRequest {
                acl_rule: "✅ allow_all".to_string(),
            }),
        )
        .await
        .into_response();
        assert_eq!(rule_resp.status(), StatusCode::OK);

        {
            let inner = store.inner.read().await;
            assert_eq!(
                inner.users.get(&test_user.sid).unwrap().acl_rule,
                "✅ allow_all"
            );
        }

        // Test update_remark_handler
        let rem_resp = update_remark_handler(
            State(state.clone()),
            Path("123456".to_string()),
            Json(RemarkRequest {
                remark: "VIP User".to_string(),
            }),
        )
        .await
        .into_response();
        assert_eq!(rem_resp.status(), StatusCode::OK);

        {
            let inner = store.inner.read().await;
            assert_eq!(inner.users.get(&test_user.sid).unwrap().remark, "VIP User");
        }

        // Test extend_user_ttl_handler
        let mut ext_headers = HeaderMap::new();
        ext_headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("fas_sid={}", test_user.sid)).unwrap(),
        );

        let initial_expire = {
            let inner = store.inner.read().await;
            inner.users.get(&test_user.sid).unwrap().expire_at
        };

        let ext_resp = extend_user_ttl_handler(
            State(state.clone()),
            ext_headers,
            Path("123456".to_string()),
        )
        .await
        .into_response();

        assert_eq!(ext_resp.status(), StatusCode::OK);
        let ext_cookie = ext_resp.headers().get(header::SET_COOKIE);
        assert!(ext_cookie.is_some());
        let ext_cookie_str = ext_cookie.unwrap().to_str().unwrap();
        assert!(ext_cookie_str.contains("fas_sid=12345678-abcd-efgh-ijkl-1234567890ab"));

        {
            let inner = store.inner.read().await;
            let extended_expire = inner.users.get(&test_user.sid).unwrap().expire_at;
            assert!(extended_expire > initial_expire);
        }

        // Test delete_user_handler
        let del_resp = delete_user_handler(State(state.clone()), Path("123456".to_string()))
            .await
            .into_response();
        assert_eq!(del_resp.status(), StatusCode::OK);

        {
            let inner = store.inner.read().await;
            assert!(!inner.users.contains_key(&test_user.sid));
        }

        if temp_data.exists() {
            let _ = std::fs::remove_file(temp_data);
        }
        if temp_acl.exists() {
            let _ = std::fs::remove_file(temp_acl);
        }
    }
}
