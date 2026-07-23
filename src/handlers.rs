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

/// GET /_health
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
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
    let s = crate::i18n::t(locale);

    let mut user_found = None;
    if !sid.is_empty() {
        let mut inner = state.store.inner.write().await;
        if let Some(user) = inner.users.get_mut(&sid) {
            user.last_ip = client_ip.clone();
            user.last_seen = Utc::now();
            user.user_agent = user_agent.clone();
            user.request_count = user.request_count.saturating_add(1);
            user.updated_at = Utc::now();
            user_found = Some(user.clone());
        }
    }

    if let Some(user) = user_found {
        if user.approved {
            state.store.mark_dirty(state.config.save_interval).await;
            return Response::builder()
                .status(StatusCode::OK)
                .body(axum::body::Body::from("Authorized"))
                .unwrap();
        }

        // Unapproved — check rate limit
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

        // Show pending page
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

    // No valid cookie — check rate limit before creating a new record
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

    // Generate a new one
    let new_sid = uuid::Uuid::new_v4().to_string();
    let new_user = crate::store::User {
        sid: new_sid.clone(),
        domain: domain.clone(),
        approved: false,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_ip: client_ip.clone(),
        last_seen: Utc::now(),
        user_agent: user_agent.clone(),
        request_count: 1,
        remark: String::new(),
    };

    {
        let mut inner = state.store.inner.write().await;
        inner.users.insert(new_sid.clone(), new_user);
    }
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

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(
            header::SET_COOKIE,
            format!(
                "fas_sid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                new_sid, state.config.cookie_max_age
            ),
        )
        .body(axum::body::Body::from(html))
        .unwrap()
}

/// GET /api/stats
pub async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.store.inner.read().await;
    let total_users = inner.users.len();
    let total_reqs: u64 = inner.users.values().map(|u| u.request_count).sum();

    Json(serde_json::json!({
        "ok": true,
        "totalUsers": total_users,
        "totalReqs": total_reqs
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

/// POST /api/users/:sid/approve
pub async fn approve_user_handler(
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
        if let Some(user) = inner.users.get_mut(&full_sid) {
            user.approved = true;
            user.updated_at = Utc::now();
            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            tracing::info!("Approved: {}", full_sid);
            return (StatusCode::OK, Json(serde_json::json!({ "ok": true })));
        }
    }
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "ok": false, "error": "User not found" })),
    )
}

/// POST /api/users/:sid/revoke
pub async fn revoke_user_handler(
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
        if let Some(user) = inner.users.get_mut(&full_sid) {
            user.approved = false;
            user.updated_at = Utc::now();
            inner.dirty = true;
            drop(inner);
            state.store.mark_dirty(state.config.save_interval).await;
            tracing::info!("Revoked: {}", full_sid);
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

/// GET / (Admin page)
pub async fn admin_page_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let locale = get_locale(&headers);
    let (users, total_users, total_reqs) = {
        let inner = state.store.inner.read().await;
        let mut users: Vec<crate::store::User> = inner.users.values().cloned().collect();
        users.sort_by_key(|u| std::cmp::Reverse(u.created_at));
        let total_users = inner.users.len();
        let total_reqs: u64 = inner.users.values().map(|u| u.request_count).sum();
        (users, total_users, total_reqs)
    };

    let table_rows = crate::templates::admin_table_rows(locale, &users);
    let html = crate::templates::admin_page(locale, &table_rows, total_users, total_reqs);

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
}
