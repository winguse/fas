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

pub fn format_relative_time(locale: Locale, dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(dt);
    let seconds = duration.num_seconds();

    let is_zh = locale == Locale::Zh;

    if seconds <= 0 {
        if is_zh {
            "刚刚".to_string()
        } else {
            "just now".to_string()
        }
    } else if seconds < 60 {
        if is_zh {
            format!("{}秒前", seconds)
        } else {
            format!("{}s ago", seconds)
        }
    } else if seconds < 3600 {
        if is_zh {
            format!("{}分钟前", seconds / 60)
        } else {
            format!("{}m ago", seconds / 60)
        }
    } else if seconds < 86400 {
        if is_zh {
            format!("{}小时前", seconds / 3600)
        } else {
            format!("{}h ago", seconds / 3600)
        }
    } else if is_zh {
        format!("{}天前", seconds / 86400)
    } else {
        format!("{}d ago", seconds / 86400)
    }
}

pub fn format_expire_time(locale: Locale, dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let seconds = dt.signed_duration_since(now).num_seconds();

    let is_zh = locale == Locale::Zh;

    if seconds <= 0 {
        if is_zh {
            "已过期".to_string()
        } else {
            "Expired".to_string()
        }
    } else if seconds < 60 {
        if is_zh {
            format!("{}秒", seconds)
        } else {
            format!("{}s", seconds)
        }
    } else if seconds < 3600 {
        let mins = (seconds + 30) / 60;
        if is_zh {
            format!("{}分钟", mins)
        } else {
            format!("{}m", mins)
        }
    } else if seconds < 86400 {
        let hours = (seconds + 1800) / 3600;
        if is_zh {
            format!("{}小时", hours)
        } else {
            format!("{}h", hours)
        }
    } else {
        let days = (seconds + 43200) / 86400;
        if is_zh {
            format!("{}天", days)
        } else {
            format!("{}d", days)
        }
    }
}

pub fn admin_table_rows(locale: Locale, users: &[User], available_rules: &[String]) -> String {
    let s = t(locale);
    if users.is_empty() {
        return format!(
            "<tr><td colspan=\"12\" class=\"empty\">{}</td></tr>",
            s.admin_empty
        );
    }

    users
        .iter()
        .map(|u| {
            let short_sid = if u.sid.len() >= 6 { &u.sid[0..6] } else { &u.sid };

            let mut options_html = String::new();
            let mut rule_found = false;
            for r in available_rules {
                let selected = if r == &u.acl_rule {
                    rule_found = true;
                    "selected"
                } else {
                    ""
                };
                options_html.push_str(&format!(
                    r#"<option value="{}" {}>{}</option>"#,
                    escape_html(r),
                    selected,
                    escape_html(r)
                ));
            }
            if !rule_found && !u.acl_rule.is_empty() {
                options_html.push_str(&format!(
                    r#"<option value="{}" selected>{}</option>"#,
                    escape_html(&u.acl_rule),
                    escape_html(&u.acl_rule)
                ));
            }

            let rule_dropdown = format!(
                r#"<select class="filter-input rule-select" style="padding:0.25rem 0.4rem; font-size:0.75rem; width:130px;" onchange="changeUserRule('{}', this.value)">{}</select>"#,
                short_sid, options_html
            );

            let last_seen_str = u.last_seen.format("%Y-%m-%d %H:%M:%S").to_string();
            let relative_seen = format_relative_time(locale, u.last_seen);
            let last_seen_display = format!("{} ({})", last_seen_str, relative_seen);

            let created_at_str = u.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
            let relative_created = format_relative_time(locale, u.created_at);
            let created_at_display = format!("{} ({})", created_at_str, relative_created);

            let expire_str = u.expire_at.format("%Y-%m-%d %H:%M:%S").to_string();
            let relative_expire = format_expire_time(locale, u.expire_at);
            let expire_display = format!(
                r#"<span title="{}">{}</span> <button class="btn btn-gray btn-sm" style="padding:0.15rem 0.4rem; font-size:0.7rem; margin-left:0.25rem;" onclick="extendTtl('{}')">{}</button>"#,
                expire_str, relative_expire, short_sid, s.btn_extend
            );

            let ip = if u.last_ip.is_empty() { "-" } else { &u.last_ip };
            let ua_short = short_ua(&u.user_agent);
            let remark_escaped = escape_html(&u.remark);

            format!(
                r#"<tr>
        <td><input type="checkbox" class="user-checkbox" data-sid="{}" onchange="toggleSelectSid('{}', this.checked)"></td>
        <td class="mono">{}</td>
        <td>{}</td>
        <td class="mono">{}</td>
        <td>{}</td>
        <td class="mono">{}</td>
        <td class="mono">{}</td>
        <td class="mono">{}</td>
        <td class="ua-cell" title="{}">{}</td>
        <td class="mono">{}</td>
        <td><input type="text" class="remark-input" data-sid="{}" onfocus="showDropdown(this)" onblur="hideDropdown(this)" oninput="handleRemarkInput(this)" onchange="updateRemark('{}', this.value)" value="{}"></td>
        <td><button class="btn btn-gray btn-sm" onclick="remove(event, '{}')">{}</button></td>
      </tr>"#,
                short_sid,
                short_sid,
                short_sid,
                escape_html(&u.last_seen_domain),
                created_at_display,
                rule_dropdown,
                escape_html(ip),
                last_seen_display,
                expire_display,
                escape_html(&u.user_agent),
                escape_html(&ua_short),
                u.request_count,
                short_sid,
                short_sid,
                remark_escaped,
                short_sid,
                s.btn_delete
            )
        })
        .collect::<Vec<String>>()
        .join("")
}

pub fn admin_page(
    locale: Locale,
    user_list: &str,
    total_users: usize,
    total_reqs: u64,
    acl_yaml: &str,
    acl_rules: &[String],
) -> String {
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
  h1 {{ font-size: 1.75rem; margin-bottom: 1rem; }}
  
  /* Tabs */
  .nav-tabs {{ display: flex; gap: 0.5rem; margin-bottom: 1.5rem; border-bottom: 1px solid #334155; padding-bottom: 0.5rem; }}
  .nav-tab {{ background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 0.5rem 1.25rem; border-radius: 8px 8px 0 0; cursor: pointer; font-size: 0.9rem; font-weight: 500; transition: all 0.15s; }}
  .nav-tab:hover {{ background: #334155; color: #e2e8f0; }}
  .nav-tab.active {{ background: #3b82f6; color: #fff; border-color: #3b82f6; }}
  
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
  .badge.clickable {{ cursor: pointer; transition: transform 0.1s, opacity 0.1s; user-select: none; }}
  .badge.clickable:hover {{ transform: scale(1.05); opacity: 0.9; }}
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
  .remark-container {{ position: relative; display: inline-block; }}
  .remark-input {{ background: #0f172a; border: 1px solid #334155; border-radius: 6px; padding: 0.25rem 0.5rem; color: #e2e8f0; font-size: 0.78rem; width: 140px; transition: border-color 0.15s; }}
  .remark-input:focus {{ outline: none; border-color: #3b82f6; }}
  .remark-dropdown {{ position: absolute; background: #1e293b; border: 1px solid #334155; border-radius: 6px; box-shadow: 0 4px 16px rgba(0,0,0,0.5); z-index: 9999; display: none; max-height: 180px; overflow-y: auto; }}
  .dropdown-item {{ padding: 0.4rem 0.6rem; color: #e2e8f0; font-size: 0.78rem; cursor: pointer; transition: background 0.1s, color 0.1s; text-align: left; }}
  .dropdown-item:hover {{ background: #0f172a; color: #38bdf8; }}
  @media (max-width: 768px) {{
    .container {{ padding: 1rem 0.5rem; }}
    th, td {{ padding: 0.4rem 0.35rem; font-size: 0.75rem; }}
    .ua-cell {{ max-width: 80px; }}
  }}
  .sortable {{ cursor: pointer; position: relative; user-select: none; }}
  .sortable:hover {{ background: #1e293b !important; color: #3b82f6; }}
  .sort-icon {{ font-size: 0.75rem; color: #3b82f6; }}
  .filter-bar {{ background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 1.25rem; margin-bottom: 1.5rem; }}
  .filter-group {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 0.75rem; align-items: center; }}
  .filter-input {{ background: #0f172a; border: 1px solid #334155; border-radius: 6px; padding: 0.4rem 0.75rem; color: #e2e8f0; font-size: 0.8rem; outline: none; transition: border-color 0.15s; width: 100%; }}
  .filter-input:focus {{ border-color: #3b82f6; }}
  .batch-bar {{ display: none; align-items: center; justify-content: space-between; background: #1e293b; border: 1px solid #3b82f6; border-radius: 8px; padding: 0.75rem 1rem; margin-bottom: 1rem; }}
  .batch-bar.show {{ display: flex; }}
  .batch-info {{ font-size: 0.85rem; color: #e2e8f0; }}
  .batch-info strong {{ color: #3b82f6; }}
  .batch-actions {{ display: flex; gap: 0.5rem; }}
  .pagination-bar {{ display: flex; align-items: center; justify-content: space-between; margin-top: 1rem; flex-wrap: wrap; gap: 1rem; }}
  .pagination-info {{ font-size: 0.82rem; color: #94a3b8; }}
  .pagination-controls {{ display: flex; gap: 0.25rem; }}
  .page-btn {{ background: #1e293b; border: 1px solid #334155; color: #94a3b8; padding: 0.35rem 0.75rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem; transition: all 0.15s; }}
  .page-btn:hover:not(.disabled) {{ background: #334155; color: #e2e8f0; border-color: #475569; }}
  .page-btn.active {{ background: #3b82f6; color: #fff; border-color: #3b82f6; }}
  .page-btn.disabled {{ opacity: 0.4; cursor: not-allowed; }}
  
  /* Config Tab Editor Styles */
  .config-container {{ background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 1.5rem; }}
  .config-toolbar {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; flex-wrap: wrap; gap: 0.75rem; }}
  .yaml-editor {{ width: 100%; height: 480px; background: #0f172a; border: 1px solid #334155; border-radius: 8px; padding: 1rem; color: #38bdf8; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 0.88rem; line-height: 1.5; outline: none; resize: vertical; }}
  .yaml-editor:focus {{ border-color: #3b82f6; }}
  .error-box {{ background: #ef444420; border: 1px solid #ef444480; color: #f87171; border-radius: 8px; padding: 0.75rem 1rem; margin-top: 1rem; font-family: monospace; font-size: 0.82rem; white-space: pre-wrap; }}
</style>
</head>
<body>
<div id="toast" class="toast"></div>
<div class="container">
<h1>{admin_heading}</h1>

<!-- Navigation Tabs -->
<div class="nav-tabs">
  <button class="nav-tab active" id="tab-btn-users" onclick="switchTab('users')">{tab_users}</button>
  <button class="nav-tab" id="tab-btn-config" onclick="switchTab('config')">{tab_config}</button>
</div>

<!-- Tab 1: Users & Visitors -->
<div id="tab-users-content">
  <div class="stats-bar">
    <span class="stat-chip">{admin_total}: <strong id="total-users">{total_users}</strong></span>
    <span class="stat-chip">{admin_total_req}: <strong id="total-reqs">{total_reqs}</strong><button class="btn btn-gray" style="padding: 0.1rem 0.4rem; margin-left: 0.5rem; font-size: 0.7rem; vertical-align: middle;" onclick="resetStats()">Reset</button></span>
  </div>

  <!-- Filter Bar -->
  <div class="filter-bar">
    <div class="filter-group">
      <input type="text" id="search-id" class="filter-input" placeholder="Search ID..." oninput="applyFilters()">
      <input type="text" id="search-domain" class="filter-input" placeholder="Search Domain..." oninput="applyFilters()">
      <select id="filter-rule" class="filter-input" onchange="applyFilters()">
        <option value="all">All ACL Rules</option>
      </select>
      <input type="text" id="search-ip" class="filter-input" placeholder="Search IP..." oninput="applyFilters()">
      <input type="text" id="search-remark" class="filter-input" placeholder="Search Remark..." oninput="applyFilters()">
      <input type="text" id="search-ua" class="filter-input" placeholder="Search User-Agent..." oninput="applyFilters()">
      <button class="btn btn-gray" style="padding: 0.4rem 0.75rem; font-size: 0.8rem; border-radius: 6px; line-height: 1.2;" onclick="clearFilters()">Reset</button>
    </div>
  </div>

  <!-- Batch Action Bar -->
  <div id="batch-bar" class="batch-bar">
    <span class="batch-info">Selected: <strong id="batch-count">0</strong> users</span>
    <div class="batch-actions" style="display:flex; align-items:center; gap:0.5rem;">
      <select id="batch-rule-select" class="filter-input" style="width: auto; padding: 0.35rem 0.5rem;">
      </select>
      <button class="btn btn-green btn-sm" onclick="batchAssignRule()">Apply Rule</button>
      <button id="btn-batch-delete" class="btn btn-gray btn-sm" onclick="batchDelete(event)">🗑️ Delete Selected</button>
    </div>
  </div>

  <div style="overflow-x:auto">
  <table>
  <thead>
  <tr>
    <th style="width: 40px;"><input type="checkbox" id="select-all-checkbox" onchange="toggleSelectAll(this)"></th>
    <th class="sortable" data-col="sid" onclick="handleSortClick('sid')">{admin_th_user}<span class="sort-icon" id="sort-icon-sid"></span></th>
    <th class="sortable" data-col="last_seen_domain" onclick="handleSortClick('last_seen_domain')">{admin_th_domain}<span class="sort-icon" id="sort-icon-last_seen_domain"></span></th>
    <th class="sortable" data-col="created_at" onclick="handleSortClick('created_at')">{admin_th_created}<span class="sort-icon" id="sort-icon-created_at"> ▼</span></th>
    <th class="sortable" data-col="acl_rule" onclick="handleSortClick('acl_rule')">{admin_th_status}<span class="sort-icon" id="sort-icon-acl_rule"></span></th>
    <th class="sortable" data-col="last_ip" onclick="handleSortClick('last_ip')">{admin_th_ip}<span class="sort-icon" id="sort-icon-last_ip"></span></th>
    <th class="sortable" data-col="last_seen" onclick="handleSortClick('last_seen')">{admin_th_last_seen}<span class="sort-icon" id="sort-icon-last_seen"></span></th>
    <th class="sortable" data-col="expire_at" onclick="handleSortClick('expire_at')">{admin_th_expires}<span class="sort-icon" id="sort-icon-expire_at"></span></th>
    <th class="sortable" data-col="user_agent" onclick="handleSortClick('user_agent')">{admin_th_ua}<span class="sort-icon" id="sort-icon-user_agent"></span></th>
    <th class="sortable" data-col="request_count" onclick="handleSortClick('request_count')">{admin_th_req_count}<span class="sort-icon" id="sort-icon-request_count"></span></th>
    <th class="sortable" data-col="remark" onclick="handleSortClick('remark')">{admin_th_remark}<span class="sort-icon" id="sort-icon-remark"></span></th>
    <th>{admin_th_actions}</th>
  </tr>
  </thead>
  <tbody id="user-list">
  {user_list}
  </tbody>
  </table>
  </div>

  <!-- Pagination Bar -->
  <div class="pagination-bar">
    <div class="pagination-info" id="pagination-info">Showing 0 to 0 of 0 entries</div>
    <div style="display: flex; align-items: center; gap: 1rem;">
      <select class="filter-input" style="width: auto; padding: 0.35rem 0.5rem;" onchange="changePageSize(this.value)">
        <option value="10">10 per page</option>
        <option value="25" selected>25 per page</option>
        <option value="50">50 per page</option>
        <option value="100">100 per page</option>
      </select>
      <div class="pagination-controls" id="pagination-controls"></div>
    </div>
  </div>
</div>

<!-- Tab 2: ACL & Cookie Configuration -->
<div id="tab-config-content" style="display:none;">
  <div class="config-container">
    <div class="config-toolbar">
      <div style="display:flex; align-items:center; gap:0.75rem;">
        <span id="config-status-badge" class="badge badge-yes">✅ Valid YAML & Regex</span>
        <span style="font-size:0.8rem; color:#94a3b8;">Edit ACL rules and Cookie domain mappings below</span>
      </div>
      <div>
        <button id="save-config-btn" class="btn btn-green" style="padding:0.45rem 1rem; font-size:0.85rem;" onclick="saveConfig()">💾 Save Configuration</button>
      </div>
    </div>

    <textarea id="yaml-editor" class="yaml-editor" spellcheck="false" oninput="validateYamlDebounced()">{acl_yaml}</textarea>
    <div id="yaml-error-box" class="error-box" style="display:none;"></div>
  </div>
</div>

</div>
<script>
const i18n = {{
  toastApproved: {toast_approved_json},
  toastRevoked: {toast_revoked_json},
  toastDeleted: {toast_deleted_json},
  toastExtended: {toast_extended_json},
  toastFailed: {toast_failed_json},
  confirmRevoke: {confirm_revoke_json},
  confirmDelete: {confirm_delete_json},
  adminEmpty: {admin_empty_json},
  badgeApproved: {badge_approved_json},
  badgePending: {badge_pending_json},
  btnRevoke: {btn_revoke_json},
  btnApprove: {btn_approve_json},
  btnDelete: {btn_delete_json},
  btnExtend: {btn_extend_json},
}};

let availableRules = {acl_rules_json};

function switchTab(tabName) {{
  const usersTabBtn = document.getElementById('tab-btn-users');
  const configTabBtn = document.getElementById('tab-btn-config');
  const usersContent = document.getElementById('tab-users-content');
  const configContent = document.getElementById('tab-config-content');

  if (tabName === 'users') {{
    usersTabBtn.classList.add('active');
    configTabBtn.classList.remove('active');
    usersContent.style.display = 'block';
    configContent.style.display = 'none';
  }} else {{
    configTabBtn.classList.add('active');
    usersTabBtn.classList.remove('active');
    configContent.style.display = 'block';
    usersContent.style.display = 'none';
  }}
}}

async function api(path, method = 'POST', body = null) {{
  const options = {{ method }};
  if (body) {{
    options.headers = {{ 'Content-Type': 'application/json' }};
    options.body = JSON.stringify(body);
  }}
  const res = await fetch(path, options);
  const data = await res.json();
  return data;
}}

function showToast(msg) {{
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.style.display = 'block';
  setTimeout(() => t.style.display = 'none', 2500);
}}

function escapeHtml(s) {{
  if (!s) return '';
  return s.replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;');
}}

function shortUa(ua) {{
  if (!ua) return '-';
  if (ua.length <= 60) return ua;
  return ua.slice(0, 57) + '...';
}}

function formatDateTime(dateStr) {{
  const d = new Date(dateStr);
  const pad = (n) => String(n).padStart(2, '0');
  
  const year = d.getUTCFullYear();
  const month = pad(d.getUTCMonth() + 1);
  const day = pad(d.getUTCDate());
  const hours = pad(d.getUTCHours());
  const minutes = pad(d.getUTCMinutes());
  const seconds = pad(d.getUTCSeconds());
  
  return year + '-' + month + '-' + day + ' ' + hours + ':' + minutes + ':' + seconds;
}}

function formatRelativeTime(dateStr) {{
  const dt = new Date(dateStr);
  const now = new Date();
  const seconds = Math.floor((now - dt) / 1000);
  const isZh = document.documentElement.lang === 'zh';

  if (seconds <= 0) {{
    return isZh ? '刚刚' : 'just now';
  }} else if (seconds < 60) {{
    return isZh ? seconds + '秒前' : seconds + 's ago';
  }} else if (seconds < 3600) {{
    const mins = Math.floor(seconds / 60);
    return isZh ? mins + '分钟前' : mins + 'm ago';
  }} else if (seconds < 86400) {{
    const hours = Math.floor(seconds / 3600);
    return isZh ? hours + '小时前' : hours + 'h ago';
  }} else {{
    const days = Math.floor(seconds / 86400);
    return isZh ? days + '天前' : days + 'd ago';
  }}
}}

function formatExpireTime(dateStr) {{
  if (!dateStr) return '-';
  const dt = new Date(dateStr);
  const now = new Date();
  const seconds = Math.floor((dt - now) / 1000);
  const isZh = document.documentElement.lang === 'zh';

  if (seconds <= 0) {{
    return isZh ? '已过期' : 'Expired';
  }} else if (seconds < 60) {{
    return isZh ? seconds + '秒' : seconds + 's';
  }} else if (seconds < 3600) {{
    const mins = Math.round(seconds / 60);
    return isZh ? mins + '分钟' : mins + 'm';
  }} else if (seconds < 86400) {{
    const hours = Math.round(seconds / 3600);
    return isZh ? hours + '小时' : hours + 'h';
  }} else {{
    const days = Math.round(seconds / 86400);
    return isZh ? days + '天' : days + 'd';
  }}
}}

function updateStats(totalUsers, totalReqs) {{
  document.getElementById('total-users').textContent = totalUsers;
  document.getElementById('total-reqs').textContent = totalReqs;
}}

const presets = [
  '🤝 Friend',
  '🏠 Family',
  '💼 Colleague',
  '👥 Relatives',
  '👑 Boss',
  '👤 Self',
  '💻 Developer',
  '⭐ VIP Client',
  '🤖 Bot',
  '🖥️ PC',
  '🍎 Mac',
  '🤖 Android',
  '📱 iPhone',
  '📟 iPad'
];

let allUsers = [];
let filteredUsers = [];
const selectedSids = new Set();
let currentSortColumn = 'created_at';
let currentSortDirection = 'desc';
let currentPage = 1;
let pageSize = 25;

let globalUsers = [];

window.activeRemarkInput = null;

function updateGlobalDropdown(input) {{
  const dropdown = document.getElementById('global-remark-dropdown');
  if (!dropdown) return;
  
  const filterText = input.value.toLowerCase().trim();
  
  const existing = new Set();
  for (const u of globalUsers) {{
    if (u.remark && u.remark.trim() !== '') {{
      existing.add(u.remark.trim());
    }}
  }}
  
  const allOptions = Array.from(new Set([...presets, ...existing]));
  
  const scoredOptions = allOptions.map((opt, originalIndex) => {{
    const optLower = opt.toLowerCase();
    let score = 0;
    
    if (filterText !== '') {{
      if (optLower === filterText) {{
        score = 100;
      }} else if (optLower.startsWith(filterText)) {{
        score = 80;
      }} else if (optLower.includes(filterText)) {{
        score = 60;
      }} else {{
        const words = optLower.split(/\s+/);
        if (words.some(w => w.startsWith(filterText))) {{
          score = 70;
        }} else if (words.some(w => w.includes(filterText))) {{
          score = 50;
        }} else {{
          score = 0;
        }}
      }}
    }}
    
    return {{ opt, score, originalIndex }};
  }});
  
  scoredOptions.sort((a, b) => {{
    if (b.score !== a.score) {{
      return b.score - a.score;
    }}
    return a.originalIndex - b.originalIndex;
  }});
  
  let html = '';
  for (const item of scoredOptions) {{
    html += '<div class="dropdown-item" onmousedown="window.selectGlobalOption(\'' + escapeHtml(item.opt) + '\')">' + escapeHtml(item.opt) + '</div>';
  }}
  
  dropdown.style.display = 'block';
  dropdown.innerHTML = html;
}}

function handleRemarkInput(input) {{
  updateGlobalDropdown(input);
}}

function showDropdown(input) {{
  window.activeRemarkInput = input;
  updateGlobalDropdown(input);
  
  const dropdown = document.getElementById('global-remark-dropdown');
  if (dropdown) {{
    const rect = input.getBoundingClientRect();
    dropdown.style.left = (rect.left + window.scrollX) + 'px';
    dropdown.style.top = (rect.bottom + window.scrollY + 2) + 'px';
    dropdown.style.minWidth = '150px';
    dropdown.style.width = rect.width + 'px';
    dropdown.classList.add('show');
  }}
}}

function hideDropdown(input) {{
  setTimeout(() => {{
    const dropdown = document.getElementById('global-remark-dropdown');
    if (dropdown) {{
      dropdown.classList.remove('show');
      dropdown.style.display = 'none';
    }}
    if (window.activeRemarkInput === input) {{
      window.activeRemarkInput = null;
    }}
  }}, 200);
}}

window.selectGlobalOption = (val) => {{
  const input = window.activeRemarkInput;
  if (input) {{
    input.value = val;
    const sid = input.getAttribute('data-sid');
    window.updateRemark(sid, val);
  }}
}};

function updateRuleSelects() {{
  const filterSelect = document.getElementById('filter-rule');
  if (filterSelect) {{
    const currentVal = filterSelect.value;
    let html = '<option value="all">All ACL Rules</option>';
    for (const r of availableRules) {{
      html += '<option value="' + escapeHtml(r) + '">' + escapeHtml(r) + '</option>';
    }}
    filterSelect.innerHTML = html;
    filterSelect.value = currentVal;
  }}

  const batchSelect = document.getElementById('batch-rule-select');
  if (batchSelect) {{
    let html = '';
    for (const r of availableRules) {{
      html += '<option value="' + escapeHtml(r) + '">' + escapeHtml(r) + '</option>';
    }}
    batchSelect.innerHTML = html;
  }}
}}

function applyFilters() {{
  const searchId = document.getElementById('search-id').value.toLowerCase().trim();
  const searchDomain = document.getElementById('search-domain').value.toLowerCase().trim();
  const filterRule = document.getElementById('filter-rule').value;
  const searchIp = document.getElementById('search-ip').value.toLowerCase().trim();
  const searchRemark = document.getElementById('search-remark').value.toLowerCase().trim();
  const searchUa = document.getElementById('search-ua').value.toLowerCase().trim();
  
  filteredUsers = allUsers.filter(u => {{
    if (searchId && !u.sid.toLowerCase().includes(searchId)) return false;
    const userDomain = u.last_seen_domain || u.domain || '';
    if (searchDomain && !userDomain.toLowerCase().includes(searchDomain)) return false;
    if (filterRule !== 'all' && u.acl_rule !== filterRule) return false;
    if (searchIp && !u.last_ip.toLowerCase().includes(searchIp)) return false;
    if (searchRemark && !u.remark.toLowerCase().includes(searchRemark)) return false;
    if (searchUa && !u.user_agent.toLowerCase().includes(searchUa)) return false;
    return true;
  }});
  
  applySort();
  renderTablePage();
  updateSelectAllCheckboxState();
}}

function clearFilters() {{
  document.getElementById('search-id').value = '';
  document.getElementById('search-domain').value = '';
  document.getElementById('filter-rule').value = 'all';
  document.getElementById('search-ip').value = '';
  document.getElementById('search-remark').value = '';
  document.getElementById('search-ua').value = '';
  applyFilters();
}}

function handleSortClick(column) {{
  if (currentSortColumn === column) {{
    currentSortDirection = currentSortDirection === 'asc' ? 'desc' : 'asc';
  }} else {{
    currentSortColumn = column;
    currentSortDirection = 'desc';
  }}
  applyFilters();
}}

function applySort() {{
  filteredUsers.sort((a, b) => {{
    let valA = a[currentSortColumn];
    let valB = b[currentSortColumn];
    
    if (valA === undefined || valA === null) valA = '';
    if (valB === undefined || valB === null) valB = '';
    
    if (typeof valA === 'string') {{
      return currentSortDirection === 'asc' 
        ? valA.localeCompare(valB) 
        : valB.localeCompare(valA);
    }}
    
    if (typeof valA === 'number' || typeof valA === 'boolean') {{
      if (valA < valB) return currentSortDirection === 'asc' ? -1 : 1;
      if (valA > valB) return currentSortDirection === 'asc' ? 1 : -1;
      return 0;
    }}
    
    return 0;
  }});
  updateSortIcons();
}}

function updateSortIcons() {{
  const ids = ['sid', 'last_seen_domain', 'created_at', 'acl_rule', 'last_ip', 'last_seen', 'expire_at', 'user_agent', 'request_count', 'remark'];
  for (const id of ids) {{
    const el = document.getElementById('sort-icon-' + id);
    if (el) {{
      if (id === currentSortColumn) {{
        el.textContent = currentSortDirection === 'asc' ? ' ▲' : ' ▼';
      }} else {{
        el.textContent = '';
      }}
    }}
  }}
}}

function renderTablePage() {{
  const tbody = document.getElementById('user-list');
  if (!filteredUsers || filteredUsers.length === 0) {{
    tbody.innerHTML = '<tr><td colspan="12" class="empty">' + escapeHtml(i18n.adminEmpty) + '</td></tr>';
    updatePaginationBar();
    updateBatchBar();
    return;
  }}
  
  const startIndex = (currentPage - 1) * pageSize;
  const endIndex = Math.min(startIndex + pageSize, filteredUsers.length);
  const pageUsers = filteredUsers.slice(startIndex, endIndex);
  
  let activeSid = null;
  let activeVal = '';
  let activeSelectionStart = 0;
  let activeSelectionEnd = 0;
  
  if (document.activeElement && document.activeElement.classList.contains('remark-input')) {{
    activeSid = document.activeElement.getAttribute('data-sid');
    activeVal = document.activeElement.value;
    activeSelectionStart = document.activeElement.selectionStart;
    activeSelectionEnd = document.activeElement.selectionEnd;
  }}
  
  let html = '';
  for (const u of pageUsers) {{
    const shortSid = u.sid;
    const isSelected = selectedSids.has(shortSid);
    
    let optionsHtml = '';
    let ruleFound = false;
    for (const r of availableRules) {{
      const isSel = (r === u.acl_rule);
      if (isSel) ruleFound = true;
      optionsHtml += '<option value="' + escapeHtml(r) + '" ' + (isSel ? 'selected' : '') + '>' + escapeHtml(r) + '</option>';
    }}
    if (!ruleFound && u.acl_rule) {{
      optionsHtml += '<option value="' + escapeHtml(u.acl_rule) + '" selected>' + escapeHtml(u.acl_rule) + '</option>';
    }}
    
    const ruleSelect = '<select class="filter-input rule-select" style="padding:0.25rem 0.4rem; font-size:0.75rem; width:130px;" onchange="changeUserRule(\'' + escapeHtml(shortSid) + '\', this.value)">' + optionsHtml + '</select>';
      
    const lastSeenStr = formatDateTime(u.last_seen);
    const relativeSeen = formatRelativeTime(u.last_seen);
    const lastSeenDisplay = lastSeenStr + ' (' + relativeSeen + ')';
    
    const createdAtStr = formatDateTime(u.created_at);
    const relativeCreated = formatRelativeTime(u.created_at);
    const createdAtDisplay = createdAtStr + ' (' + relativeCreated + ')';
    
    const expireStr = formatDateTime(u.expire_at);
    const relativeExpire = formatExpireTime(u.expire_at);
    const expireDisplay = '<span title="' + escapeHtml(expireStr) + '">' + escapeHtml(relativeExpire) + '</span> ' +
      '<button class="btn btn-gray btn-sm" style="padding:0.15rem 0.4rem; font-size:0.7rem; margin-left:0.25rem;" onclick="extendTtl(\'' + escapeHtml(shortSid) + '\')">' + escapeHtml(i18n.btnExtend) + '</button>';

    const ip = u.last_ip || '-';
    const uaShort = shortUa(u.user_agent);
    const remarkVal = (shortSid === activeSid) ? activeVal : (u.remark || '');
    const displayDomain = u.last_seen_domain || u.domain || '-';
    
    html += '<tr>' +
      '<td><input type="checkbox" class="user-checkbox" data-sid="' + escapeHtml(shortSid) + '" ' + (isSelected ? 'checked' : '') + ' onchange="toggleSelectSid(\'' + escapeHtml(shortSid) + '\', this.checked)"></td>' +
      '<td class="mono">' + escapeHtml(shortSid) + '</td>' +
      '<td>' + escapeHtml(displayDomain) + '</td>' +
      '<td class="mono">' + escapeHtml(createdAtDisplay) + '</td>' +
      '<td>' + ruleSelect + '</td>' +
      '<td class="mono">' + escapeHtml(ip) + '</td>' +
      '<td class="mono">' + escapeHtml(lastSeenDisplay) + '</td>' +
      '<td class="mono">' + expireDisplay + '</td>' +
      '<td class="ua-cell" title="' + escapeHtml(u.user_agent) + '">' + escapeHtml(uaShort) + '</td>' +
      '<td class="mono">' + u.request_count + '</td>' +
      '<td>' +
        '<input type="text" class="remark-input" data-sid="' + escapeHtml(shortSid) + '" ' +
          'onfocus="showDropdown(this)" onblur="hideDropdown(this)" oninput="handleRemarkInput(this)" ' +
          'onchange="updateRemark(\'' + escapeHtml(shortSid) + '\', this.value)" value="' + escapeHtml(remarkVal) + '">' +
      '</td>' +
      '<td><button class="btn btn-gray btn-sm" onclick="remove(event, \'' + escapeHtml(shortSid) + '\')">' + escapeHtml(i18n.btnDelete) + '</button></td>' +
    '</tr>';
  }}
  tbody.innerHTML = html;
  
  if (activeSid) {{
    const input = document.querySelector('.remark-input[data-sid="' + activeSid + '"]');
    if (input) {{
      input.focus();
      input.value = activeVal;
      input.setSelectionRange(activeSelectionStart, activeSelectionEnd);
      showDropdown(input);
    }}
  }}
  
  updatePaginationBar();
  updateBatchBar();
}}

function toggleSelectSid(sid, checked) {{
  if (checked) {{
    selectedSids.add(sid);
  }} else {{
    selectedSids.delete(sid);
  }}
  updateSelectAllCheckboxState();
  updateBatchBar();
}}

function toggleSelectAll(checkbox) {{
  const visibleSids = getVisibleSids();
  if (checkbox.checked) {{
    for (const sid of visibleSids) {{
      selectedSids.add(sid);
    }}
  }} else {{
    for (const sid of visibleSids) {{
      selectedSids.delete(sid);
    }}
  }}
  renderTablePage();
}}

function getVisibleSids() {{
  const startIndex = (currentPage - 1) * pageSize;
  const endIndex = Math.min(startIndex + pageSize, filteredUsers.length);
  return filteredUsers.slice(startIndex, endIndex).map(u => u.sid);
}}

function updateSelectAllCheckboxState() {{
  const checkbox = document.getElementById('select-all-checkbox');
  if (!checkbox) return;
  const visibleSids = getVisibleSids();
  if (visibleSids.length === 0) {{
    checkbox.checked = false;
    checkbox.indeterminate = false;
    return;
  }}
  
  let allSelected = true;
  let noneSelected = true;
  for (const sid of visibleSids) {{
    if (selectedSids.has(sid)) {{
      noneSelected = false;
    }} else {{
      allSelected = false;
    }}
  }}
  
  checkbox.checked = allSelected;
  checkbox.indeterminate = !allSelected && !noneSelected;
}}

function changePage(page) {{
  const totalPages = Math.ceil(filteredUsers.length / pageSize);
  if (page < 1 || page > totalPages) return;
  currentPage = page;
  renderTablePage();
  updateSelectAllCheckboxState();
}}

function changePageSize(size) {{
  pageSize = parseInt(size, 10);
  currentPage = 1;
  renderTablePage();
  updateSelectAllCheckboxState();
}}

function updatePaginationBar() {{
  const totalEntries = filteredUsers.length;
  const totalPages = Math.ceil(totalEntries / pageSize);
  
  const infoEl = document.getElementById('pagination-info');
  if (infoEl) {{
    if (totalEntries === 0) {{
      infoEl.textContent = 'Showing 0 to 0 of 0 entries';
    }} else {{
      const start = (currentPage - 1) * pageSize + 1;
      const end = Math.min(start + pageSize - 1, totalEntries);
      infoEl.textContent = 'Showing ' + start + ' to ' + end + ' of ' + totalEntries + ' entries';
    }}
  }}
  
  const controlsEl = document.getElementById('pagination-controls');
  if (!controlsEl) return;
  
  if (totalEntries === 0) {{
    controlsEl.innerHTML = '';
    return;
  }}
  
  let html = '';
  html += '<button class="page-btn ' + (currentPage === 1 ? 'disabled' : '') + '" onclick="changePage(' + (currentPage - 1) + ')">Prev</button>';
  
  const range = 2;
  for (let i = 1; i <= totalPages; i++) {{
    if (i === 1 || i === totalPages || (i >= currentPage - range && i <= currentPage + range)) {{
      html += '<button class="page-btn ' + (i === currentPage ? 'active' : '') + '" onclick="changePage(' + i + ')">' + i + '</button>';
    }} else if (i === currentPage - range - 1 || i === currentPage + range + 1) {{
      html += '<span class="page-btn disabled">...</span>';
    }}
  }}
  
  html += '<button class="page-btn ' + (currentPage === totalPages ? 'disabled' : '') + '" onclick="changePage(' + (currentPage + 1) + ')">Next</button>';
  
  controlsEl.innerHTML = html;
}}

function updateBatchBar() {{
  const bar = document.getElementById('batch-bar');
  const countEl = document.getElementById('batch-count');
  if (bar && countEl) {{
    if (selectedSids.size > 0) {{
      countEl.textContent = selectedSids.size;
      bar.classList.add('show');
    }} else {{
      bar.classList.remove('show');
      if (confirmStates['batch_delete']) {{
        confirmStates['batch_delete'].reset();
      }}
    }}
  }}
}}

async function changeUserRule(sid, rule) {{
  const isZh = document.documentElement.lang === 'zh';
  const data = await api('/api/users/' + encodeURIComponent(sid) + '/rule', 'POST', {{ acl_rule: rule }});
  if (data.ok) {{
    showToast(isZh ? 'ACL 规则已更新' : 'ACL Rule updated');
    await loadData();
  }} else {{
    showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
  }}
}}

async function batchAssignRule() {{
  if (selectedSids.size === 0) return;
  const rule = document.getElementById('batch-rule-select').value;
  const sids = Array.from(selectedSids);
  const isZh = document.documentElement.lang === 'zh';
  showToast(isZh ? '正在更新 ACL 规则...' : 'Updating ACL rules...');
  
  try {{
    const promises = sids.map(sid => api('/api/users/' + encodeURIComponent(sid) + '/rule', 'POST', {{ acl_rule: rule }}));
    const results = await Promise.all(promises);
    const successCount = results.filter(r => r.ok).length;
    showToast(isZh ? '成功更新了 ' + successCount + ' 个用户的 ACL 规则' : 'Successfully updated ' + successCount + ' users');
    selectedSids.clear();
    await loadData();
  }} catch (err) {{
    showToast(i18n.toastFailed);
  }}
}}

async function batchDelete(event) {{
  if (selectedSids.size === 0) return;
  const sids = Array.from(selectedSids);
  const isZh = document.documentElement.lang === 'zh';
  const confirmText = isZh ? '确认批量删除' : 'Confirm Delete Selected';
  
  await handleConfirm(event, 'batch_delete', confirmText, async () => {{
    showToast(isZh ? '正在批量删除...' : 'Batch deleting...');
    try {{
      const promises = sids.map(sid => api('/api/users/' + encodeURIComponent(sid), 'DELETE'));
      const results = await Promise.all(promises);
      const successCount = results.filter(r => r.ok).length;
      showToast(isZh ? '成功删除了 ' + successCount + ' 个用户' : 'Successfully deleted ' + successCount + ' users');
      selectedSids.clear();
      await loadData();
    }} catch (err) {{
      showToast(i18n.toastFailed);
    }}
  }});
}}

window.resetStats = async () => {{
  const isZh = document.documentElement.lang === 'zh';
  const confirmText = isZh ? '确定要重置请求计数器吗？' : 'Are you sure you want to reset the request counter?';
  if (!confirm(confirmText)) return;
  
  const data = await api('/api/stats/reset', 'POST');
  if (data.ok) {{
    document.getElementById('total-reqs').textContent = '0';
    showToast(isZh ? '请求计数器已重置' : 'Request counter reset');
  }} else {{
    showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
  }}
}};

let validateTimer = null;

function validateYamlDebounced() {{
  clearTimeout(validateTimer);
  validateTimer = setTimeout(performYamlValidation, 300);
}}

async function performYamlValidation() {{
  const editor = document.getElementById('yaml-editor');
  const badge = document.getElementById('config-status-badge');
  const errBox = document.getElementById('yaml-error-box');
  const saveBtn = document.getElementById('save-config-btn');
  const isZh = document.documentElement.lang === 'zh';

  if (!editor) return;
  const yamlText = editor.value;

  try {{
    const res = await api('/api/config/validate', 'POST', {{ yaml: yamlText }});
    if (res.ok) {{
      badge.textContent = isZh ? '✅ 语法正确 (YAML & Regex)' : '✅ Valid YAML & Regex';
      badge.className = 'badge badge-yes';
      errBox.style.display = 'none';
      errBox.textContent = '';
      if (saveBtn) saveBtn.disabled = false;
      if (res.rules && Array.isArray(res.rules)) {{
        availableRules = res.rules;
        updateRuleSelects();
      }}
    }} else {{
      badge.textContent = isZh ? '❌ 语法或正则错误' : '❌ Syntax / Regex Error';
      badge.className = 'badge badge-no';
      errBox.style.display = 'block';
      errBox.textContent = res.error || 'Unknown error';
      if (saveBtn) saveBtn.disabled = true;
    }}
  }} catch (err) {{
    badge.textContent = isZh ? '❌ 校验失败' : '❌ Validation Failed';
    badge.className = 'badge badge-no';
    errBox.style.display = 'block';
    errBox.textContent = String(err);
    if (saveBtn) saveBtn.disabled = true;
  }}
}}

async function saveConfig() {{
  const editor = document.getElementById('yaml-editor');
  const isZh = document.documentElement.lang === 'zh';
  if (!editor) return;

  showToast(isZh ? '正在保存配置...' : 'Saving configuration...');
  const res = await api('/api/config', 'POST', {{ yaml: editor.value }});
  if (res.ok) {{
    showToast(isZh ? '✅ 配置保存成功' : '✅ Configuration saved');
    await loadData();
  }} else {{
    showToast((isZh ? '❌ 保存失败: ' : '❌ Save failed: ') + (res.error || ''));
  }}
}}

async function loadData() {{
  try {{
    const [usersRes, statsRes, configRes] = await Promise.all([
      api('/api/users', 'GET'),
      api('/api/stats', 'GET'),
      api('/api/config', 'GET')
    ]);
    
    if (configRes.ok && configRes.rules) {{
      availableRules = configRes.rules;
      updateRuleSelects();
    }}
    
    if (usersRes.ok && statsRes.ok) {{
      updateStats(statsRes.totalUsers, statsRes.totalReqs);
      allUsers = usersRes.users;
      globalUsers = allUsers;
      applyFilters();
    }}
  }} catch (err) {{
    console.error('Failed to load data:', err);
  }}
}}

const confirmStates = {{}};

function startConfirm(btn, key, originalText, confirmText, callback) {{
  let seconds = 10;
  btn.textContent = confirmText + ' (' + seconds + 's)';
  btn.classList.add('confirming');
  
  const timer = setInterval(() => {{
    seconds--;
    if (seconds <= 0) {{
      clearInterval(timer);
      btn.textContent = originalText;
      btn.classList.remove('confirming');
      btn.removeAttribute('data-original-text');
      delete confirmStates[key];
    }} else {{
      btn.textContent = confirmText + ' (' + seconds + 's)';
    }}
  }}, 1000);
  
  confirmStates[key] = {{
    timer,
    reset: () => {{
      clearInterval(timer);
      btn.textContent = originalText;
      btn.classList.remove('confirming');
      btn.removeAttribute('data-original-text');
      delete confirmStates[key];
    }}
  }};
}}

async function handleConfirm(event, key, confirmText, callback) {{
  const btn = event.currentTarget || event.target;
  const originalText = btn.getAttribute('data-original-text') || btn.textContent;
  if (!btn.getAttribute('data-original-text')) {{
    btn.setAttribute('data-original-text', originalText);
  }}
  
  if (confirmStates[key]) {{
    const state = confirmStates[key];
    clearInterval(state.timer);
    delete confirmStates[key];
    btn.textContent = originalText;
    btn.classList.remove('confirming');
    btn.removeAttribute('data-original-text');
    await callback();
  }} else {{
    for (const k in confirmStates) {{
      confirmStates[k].reset();
    }}
    startConfirm(btn, key, originalText, confirmText, callback);
  }}
}}

window.remove = async (event, sid) => {{
  const isZh = document.documentElement.lang === 'zh';
  const confirmText = isZh ? '确认删除' : 'Confirm Delete';
  await handleConfirm(event, 'delete_' + sid, confirmText, async () => {{
    const data = await api('/api/users/' + encodeURIComponent(sid), 'DELETE');
    if (data.ok) {{
      showToast(i18n.toastDeleted);
      await loadData();
    }} else {{
      showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
    }}
  }});
}};

window.updateRemark = async (sid, val) => {{
  const isZh = document.documentElement.lang === 'zh';
  const data = await api('/api/users/' + encodeURIComponent(sid) + '/remark', 'POST', {{ remark: val }});
  if (data.ok) {{
    showToast(isZh ? '备注已更新' : 'Remark updated');
    await loadData();
  }} else {{
    showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
  }}
}};

window.extendTtl = async (sid) => {{
  const isZh = document.documentElement.lang === 'zh';
  const data = await api('/api/users/' + encodeURIComponent(sid) + '/extend', 'POST');
  if (data.ok) {{
    showToast(i18n.toastExtended || (isZh ? '已延长有效期' : 'TTL extended'));
    await loadData();
  }} else {{
    showToast(i18n.toastFailed + (data.error ? ': ' + data.error : ''));
  }}
}};

updateRuleSelects();
loadData();

// Auto reload every 10s if the tab is active, no confirmation is pending, and no input is focused
setInterval(() => {{
  const isEditing = document.activeElement && (document.activeElement.classList.contains('remark-input') || document.activeElement.id === 'yaml-editor');
  if (!document.hidden && Object.keys(confirmStates).length === 0 && !isEditing) {{
    loadData();
  }}
}}, 10000);
</script>
<div id="global-remark-dropdown" class="remark-dropdown"></div>
</body>
</html>"#,
        lang_attr = lang_attr,
        admin_title = s.admin_title,
        admin_heading = s.admin_heading,
        tab_users = s.tab_users,
        tab_config = s.tab_config,
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
        admin_th_expires = s.admin_th_expires,
        admin_th_ua = s.admin_th_ua,
        admin_th_req_count = s.admin_th_req_count,
        admin_th_remark = s.admin_th_remark,
        admin_th_actions = s.admin_th_actions,
        user_list = user_list,
        acl_yaml = escape_html(acl_yaml),
        toast_approved_json = serde_json::to_string(s.toast_approved).unwrap(),
        toast_revoked_json = serde_json::to_string(s.toast_revoked).unwrap(),
        toast_deleted_json = serde_json::to_string(s.toast_deleted).unwrap(),
        toast_extended_json = serde_json::to_string(s.toast_extended).unwrap(),
        toast_failed_json = serde_json::to_string(s.toast_failed).unwrap(),
        confirm_revoke_json = serde_json::to_string(s.confirm_revoke).unwrap(),
        confirm_delete_json = serde_json::to_string(s.confirm_delete).unwrap(),
        admin_empty_json = serde_json::to_string(s.admin_empty).unwrap(),
        badge_approved_json = serde_json::to_string(s.badge_approved).unwrap(),
        badge_pending_json = serde_json::to_string(s.badge_pending).unwrap(),
        btn_revoke_json = serde_json::to_string(s.btn_revoke).unwrap(),
        btn_approve_json = serde_json::to_string(s.btn_approve).unwrap(),
        btn_delete_json = serde_json::to_string(s.btn_delete).unwrap(),
        btn_extend_json = serde_json::to_string(s.btn_extend).unwrap(),
        acl_rules_json = serde_json::to_string(acl_rules).unwrap(),
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
  .id-box {{ background: #0f172a; border: 1px solid #334155; border-radius: 8px; padding: 1rem; margin: 1rem 0; font-family: monospace; font-size: 0.9rem; word-break: break-all; color: #38bdf8; display: flex; align-items: center; justify-content: center; position: relative; }}
  .id-box span {{ text-align: center; }}
  .copy-btn {{ position: absolute; right: 1rem; background: #1e293b; border: 1px solid #3b82f6; color: #3b82f6; border-radius: 6px; padding: 0.35rem 0.75rem; cursor: pointer; font-size: 0.8rem; white-space: nowrap; transition: all 0.15s; }}
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
(function() {{
  const isZh = document.documentElement.lang === 'zh-CN';
  const msgCheckingIn = isZh ? '将在 {{{{seconds}}}} 秒后自动检查...' : 'Checking in {{{{seconds}}}}s...';
  const msgChecking = isZh ? '正在检查...' : 'Checking...';
  const msgAuthenticated = isZh ? '认证成功！正在刷新...' : 'Authenticated! Refreshing...';
  
  const statusEl = document.getElementById('checkStatus');
  if (!statusEl) return;
  
  let countdown = 10;
  
  function updateText() {{
    statusEl.textContent = msgCheckingIn.replace('{{{{seconds}}}}', countdown);
  }}
  
  async function performCheck() {{
    statusEl.textContent = msgChecking;
    try {{
      const res = await fetch(window.location.href, {{ method: 'HEAD' }});
      if (res.status !== 401 && res.status !== 429) {{
        statusEl.textContent = msgAuthenticated;
        statusEl.style.color = '#22c55e';
        setTimeout(() => {{
          location.reload();
        }}, 1000);
        return true;
      }}
    }} catch (e) {{}}
    return false;
  }}
  
  updateText();
  
  setInterval(async () => {{
    if (document.hidden) return;
    
    countdown--;
    if (countdown <= 0) {{
      const success = await performCheck();
      if (!success) {{
        countdown = 10;
        updateText();
      }} else {{
        countdown = 999999;
      }}
    }} else if (countdown < 99999) {{
      updateText();
    }}
  }}, 1000);
}})();
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_format_relative_time() {
        let now = Utc::now();
        assert_eq!(format_relative_time(Locale::En, now), "just now");
        assert_eq!(
            format_relative_time(Locale::En, now - Duration::seconds(10)),
            "10s ago"
        );
        assert_eq!(
            format_relative_time(Locale::En, now - Duration::minutes(5)),
            "5m ago"
        );
        assert_eq!(
            format_relative_time(Locale::En, now - Duration::hours(2)),
            "2h ago"
        );
        assert_eq!(
            format_relative_time(Locale::En, now - Duration::days(3)),
            "3d ago"
        );

        assert_eq!(format_relative_time(Locale::Zh, now), "刚刚");
        assert_eq!(
            format_relative_time(Locale::Zh, now - Duration::seconds(10)),
            "10秒前"
        );
        assert_eq!(
            format_relative_time(Locale::Zh, now - Duration::minutes(5)),
            "5分钟前"
        );
        assert_eq!(
            format_relative_time(Locale::Zh, now - Duration::hours(2)),
            "2小时前"
        );
        assert_eq!(
            format_relative_time(Locale::Zh, now - Duration::days(3)),
            "3天前"
        );
    }

    #[test]
    fn test_format_expire_time() {
        assert_eq!(
            format_expire_time(Locale::En, Utc::now() - Duration::seconds(10)),
            "Expired"
        );
        let res_s = format_expire_time(Locale::En, Utc::now() + Duration::seconds(30));
        assert!(res_s == "30s" || res_s == "29s");
        assert_eq!(
            format_expire_time(Locale::En, Utc::now() + Duration::minutes(15)),
            "15m"
        );
        assert_eq!(
            format_expire_time(Locale::En, Utc::now() + Duration::hours(4)),
            "4h"
        );
        assert_eq!(
            format_expire_time(Locale::En, Utc::now() + Duration::days(90)),
            "90d"
        );

        assert_eq!(
            format_expire_time(Locale::Zh, Utc::now() - Duration::seconds(10)),
            "已过期"
        );
        let res_zh_s = format_expire_time(Locale::Zh, Utc::now() + Duration::seconds(30));
        assert!(res_zh_s == "30秒" || res_zh_s == "29秒");
        assert_eq!(
            format_expire_time(Locale::Zh, Utc::now() + Duration::minutes(15)),
            "15分钟"
        );
        assert_eq!(
            format_expire_time(Locale::Zh, Utc::now() + Duration::hours(4)),
            "4小时"
        );
        assert_eq!(
            format_expire_time(Locale::Zh, Utc::now() + Duration::days(90)),
            "90天"
        );
    }
}
