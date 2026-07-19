use crate::i18n::{t, Locale};
use crate::store::User;

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn short_ua(ua: &str) -> String {
    if ua.is_empty() {
        return "-".to_string();
    }
    if ua.chars().count() <= 60 {
        ua.to_string()
    } else {
        let mut truncated: String = ua.chars().take(57).collect();
        truncated.push_str("...");
        truncated
    }
}

pub fn admin_table_rows(locale: Locale, users: &[User]) -> String {
    let s = t(locale);
    if users.is_empty() {
        return format!(
            "<tr><td colspan=\"9\" class=\"empty\">{}</td></tr>",
            s.admin_empty
        );
    }

    users
        .iter()
        .map(|u| {
            let status_badge = if u.approved {
                format!("<span class=\"badge badge-yes\">✅ {}</span>", s.badge_approved)
            } else {
                format!("<span class=\"badge badge-no\">⏳ {}</span>", s.badge_pending)
            };

            let approve_btn = if u.approved {
                format!(
                    "<button class=\"btn btn-red btn-sm\" onclick=\"revoke('{}')\">{}</button>",
                    u.sid, s.btn_revoke
                )
            } else {
                format!(
                    "<button class=\"btn btn-green btn-sm\" onclick=\"approve('{}')\">{}</button>",
                    u.sid, s.btn_approve
                )
            };

            let last_seen_str = u.last_seen.format("%Y-%m-%d %H:%M:%S").to_string();
            let created_at_str = u.created_at.format("%Y-%m-%d %H:%M:%S").to_string();

            let ip = if u.last_ip.is_empty() { "-" } else { &u.last_ip };
            let ua_short = short_ua(&u.user_agent);

            format!(
                r#"<tr>
        <td class="mono">{}…</td>
        <td>{}</td>
        <td class="mono">{}</td>
        <td>{}</td>
        <td class="mono">{}</td>
        <td class="mono">{}</td>
        <td class="ua-cell" title="{}">{}</td>
        <td class="mono">{}</td>
        <td><div class="actions">{}<button class="btn btn-gray btn-sm" onclick="remove('{}')">{}</button></div></td>
      </tr>"#,
                if u.sid.len() >= 8 { &u.sid[0..8] } else { &u.sid },
                escape_html(&u.domain),
                created_at_str,
                status_badge,
                escape_html(ip),
                last_seen_str,
                escape_html(&u.user_agent),
                escape_html(&ua_short),
                u.request_count,
                approve_btn,
                u.sid,
                s.btn_delete
            )
        })
        .collect::<Vec<String>>()
        .join("")
}

pub fn admin_page(locale: Locale, user_list: &str, total_users: usize, total_reqs: u64) -> String {
    let s = t(locale);
    let lang_attr = locale.html_lang();

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang_attr}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{admin_title}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; }}
  .container {{ padding: 2rem; }}
  h1 {{ font-size: 1.75rem; margin-bottom: 0.5rem; }}
  .stats-bar {{ display: flex; gap: 1.5rem; margin-bottom: 1.5rem; flex-wrap: wrap; }}
  .stat-chip {{ background: #1e293b; border: 1px solid #334155; border-radius: 8px; padding: 0.5rem 1rem; font-size: 0.85rem; }}
  .stat-chip strong {{ color: #38bdf8; }}
  table {{ width: 100%; border-collapse: collapse; background: #1e293b; border-radius: 12px; overflow: hidden; }}
  th, td {{ padding: 0.6rem 0.75rem; text-align: left; border-bottom: 1px solid #334155; }}
  th {{ background: #0f172a; font-weight: 600; color: #94a3b8; font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.05em; }}
  tr:hover td {{ background: #1e293b; }}
  .badge {{ display: inline-block; padding: 0.2rem 0.6rem; border-radius: 999px; font-size: 0.75rem; font-weight: 600; }}
  .badge-yes {{ background: #22c55e20; color: #22c55e; border: 1px solid #22c55e40; }}
  .badge-no {{ background: #ef444420; color: #ef4444; border: 1px solid #ef444440; }}
  .btn {{ display: inline-block; padding: 0.3rem 0.6rem; border: none; border-radius: 6px; font-size: 0.75rem; cursor: pointer; font-weight: 500; transition: opacity 0.15s; }}
  .btn:hover {{ opacity: 0.8; }}
  .btn-green {{ background: #22c55e; color: #fff; }}
  .btn-red {{ background: #ef4444; color: #fff; }}
  .btn-gray {{ background: #475569; color: #fff; }}
  .mono {{ font-family: monospace; font-size: 0.78rem; }}
  .actions {{ display: flex; gap: 0.3rem; }}
  .empty {{ text-align: center; padding: 3rem 1rem; color: #64748b; }}
  .toast {{ position: fixed; top: 1rem; right: 1rem; background: #1e293b; border: 1px solid #334155; border-radius: 8px; padding: 0.75rem 1.25rem; color: #e2e8f0; font-size: 0.9rem; box-shadow: 0 4px 12px rgba(0,0,0,0.3); display: none; z-index: 100; }}
  .ua-cell {{ max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  @media (max-width: 768px) {{
    .container {{ padding: 1rem 0.5rem; }}
    th, td {{ padding: 0.4rem 0.35rem; font-size: 0.75rem; }}
    .ua-cell {{ max-width: 80px; }}
  }}
</style>
</head>
<body>
<div id="toast" class="toast"></div>
<div class="container">
<h1>{admin_heading}</h1>
<div class="stats-bar">
  <span class="stat-chip">{admin_total}: <strong>{total_users}</strong></span>
  <span class="stat-chip">{admin_total_req}: <strong>{total_reqs}</strong></span>
</div>
<div style="overflow-x:auto">
<table>
<thead>
<tr>
  <th>{admin_th_user}</th>
  <th>{admin_th_domain}</th>
  <th>{admin_th_created}</th>
  <th>{admin_th_status}</th>
  <th>{admin_th_ip}</th>
  <th>{admin_th_last_seen}</th>
  <th>{admin_th_ua}</th>
  <th>{admin_th_req_count}</th>
  <th>{admin_th_actions}</th>
</tr>
</thead>
<tbody id="user-list">
{user_list}
</tbody>
</table>
</div>
</div>
<script>
const i18n = {{
  toastApproved: {toast_approved_json},
  toastRevoked: {toast_revoked_json},
  toastDeleted: {toast_deleted_json},
  toastFailed: {toast_failed_json},
  confirmRevoke: {confirm_revoke_json},
  confirmDelete: {confirm_delete_json},
}};
async function api(path, method = 'POST') {{
  const res = await fetch(path, {{ method }});
  const data = await res.json();
  return data;
}}
function showToast(msg) {{
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.style.display = 'block';
  setTimeout(() => t.style.display = 'none', 2500);
}}
window.approve = async (sid) => {{
  const data = await api('/api/users/' + encodeURIComponent(sid) + '/approve');
  if (data.ok) {{ showToast(i18n.toastApproved); location.reload(); }}
  else showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
}};
window.revoke = async (sid) => {{
  if (!confirm(i18n.confirmRevoke)) return;
  const data = await api('/api/users/' + encodeURIComponent(sid) + '/revoke');
  if (data.ok) {{ showToast(i18n.toastRevoked); location.reload(); }}
  else showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
}};
window.remove = async (sid) => {{
  if (!confirm(i18n.confirmDelete)) return;
  const data = await api('/api/users/' + encodeURIComponent(sid), 'DELETE');
  if (data.ok) {{ showToast(i18n.toastDeleted); location.reload(); }}
  else showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
}};
</script>
</body>
</html>"#,
        lang_attr = lang_attr,
        admin_title = s.admin_title,
        admin_heading = s.admin_heading,
        admin_total = s.admin_total,
        total_users = total_users,
        admin_total_req = s.admin_total_req,
        total_reqs = total_reqs,
        admin_th_user = s.admin_th_user,
        admin_th_domain = s.admin_th_domain,
        admin_th_created = s.admin_th_created,
        admin_th_status = s.admin_th_status,
        admin_th_ip = s.admin_th_ip,
        admin_th_last_seen = s.admin_th_last_seen,
        admin_th_ua = s.admin_th_ua,
        admin_th_req_count = s.admin_th_req_count,
        admin_th_actions = s.admin_th_actions,
        user_list = user_list,
        toast_approved_json = serde_json::to_string(s.toast_approved).unwrap(),
        toast_revoked_json = serde_json::to_string(s.toast_revoked).unwrap(),
        toast_deleted_json = serde_json::to_string(s.toast_deleted).unwrap(),
        toast_failed_json = serde_json::to_string(s.toast_failed).unwrap(),
        confirm_revoke_json = serde_json::to_string(s.confirm_revoke).unwrap(),
        confirm_delete_json = serde_json::to_string(s.confirm_delete).unwrap(),
    )
}

pub fn visitor_page(locale: Locale, title: &str, body: &str) -> String {
    let lang_attr = locale.html_lang();
    let s = t(locale);
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang_attr}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; }}
  .card {{ background: #1e293b; border-radius: 12px; padding: 2.5rem; max-width: 480px; width: 90%; box-shadow: 0 4px 24px rgba(0,0,0,0.3); text-align: center; }}
  h1 {{ font-size: 1.5rem; margin-bottom: 0.5rem; }}
  p {{ color: #94a3b8; margin: 0.5rem 0; line-height: 1.6; }}
  .id-box {{ background: #0f172a; border: 1px solid #334155; border-radius: 8px; padding: 1rem; margin: 1rem 0; font-family: monospace; font-size: 0.9rem; word-break: break-all; color: #38bdf8; display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; }}
  .id-box span {{ flex: 1; text-align: left; }}
  .copy-btn {{ background: #1e293b; border: 1px solid #3b82f6; color: #3b82f6; border-radius: 6px; padding: 0.35rem 0.75rem; cursor: pointer; font-size: 0.8rem; white-space: nowrap; transition: all 0.15s; }}
  .copy-btn:hover {{ background: #3b82f6; color: #fff; }}
  .copy-btn.done {{ background: #22c55e; border-color: #22c55e; color: #fff; }}
  .badge {{ display: inline-block; padding: 0.25rem 0.75rem; border-radius: 999px; font-size: 0.8rem; font-weight: 600; }}
  .badge-warn {{ background: #f59e0b20; color: #f59e0b; border: 1px solid #f59e0b40; }}
  .badge-ok {{ background: #22c55e20; color: #22c55e; border: 1px solid #22c55e40; }}
  footer {{ margin-top: 2rem; font-size: 0.75rem; color: #475569; }}
</style>
</head>
<body>
<div class="card">
{body}
<footer>FAS v1</footer>
</div>
<script>
function copyId() {{
  const idText = document.getElementById('visitorId').textContent;
  navigator.clipboard.writeText(idText).then(() => {{
    const btn = document.querySelector('.copy-btn');
    const oldText = btn.textContent;
    btn.textContent = '{copy_btn_done}';
    btn.classList.add('done');
    setTimeout(() => {{
      btn.textContent = oldText;
      btn.classList.remove('done');
    }}, 2000);
  }});
}}
</script>
</body>
</html>"#,
        lang_attr = lang_attr,
        title = title,
        body = body,
        copy_btn_done = s.copied
    )
}

pub fn rate_limit_page(locale: Locale, retry_after: u64, _ip: &str) -> String {
    let s = t(locale);
    let lang_attr = locale.html_lang();
    let refresh_label = if locale == Locale::Zh {
        "刷新"
    } else {
        "Refresh"
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang_attr}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{rate_limit_title}</title>
<style>
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #e2e8f0; min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; }}
  .card {{ background: #1e293b; border-radius: 12px; padding: 2.5rem; max-width: 480px; width: 90%; box-shadow: 0 4px 24px rgba(0,0,0,0.3); text-align: center; }}
  h1 {{ font-size: 1.5rem; margin-bottom: 0.5rem; }}
  p {{ color: #94a3b8; margin: 0.5rem 0; line-height: 1.6; }}
  .countdown-btn {{ display: inline-block; margin-top: 1.5rem; padding: 0.6rem 1.5rem; border: none; border-radius: 8px; background: #334155; color: #e2e8f0; font-size: 1rem; cursor: not-allowed; }}
  .countdown-btn.active {{ background: #3b82f6; cursor: pointer; }}
  .countdown-btn.active:hover {{ background: #2563eb; }}
</style>
</head>
<body>
<div class="card">
<h1>⏱️ {rate_limit_title}</h1>
<p>{rate_limit_body}</p>
<button id="retryBtn" class="countdown-btn" disabled>{rate_limit_btn} {retry_after}s</button>
</div>
<script>
(function(){{let n={retry_after};const btn=document.getElementById('retryBtn');const iv=setInterval(()=>{{n--;if(n<=0){{clearInterval(iv);btn.textContent='{refresh_label}';btn.disabled=false;btn.classList.add('active');btn.onclick=()=>location.reload();}}else{{btn.textContent='{rate_limit_btn} '+n+'s';}}}},1000);}})();
</script>
</body>
</html>"#,
        lang_attr = lang_attr,
        rate_limit_title = s.rate_limit_title,
        rate_limit_body = s.rate_limit_body,
        rate_limit_btn = s.rate_limit_btn,
        retry_after = retry_after,
        refresh_label = refresh_label
    )
}
