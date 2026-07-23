use regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Zh,
}

impl Locale {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Zh => "zh",
        }
    }

    pub fn html_lang(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Zh => "zh-CN",
        }
    }
}

pub struct I18nStrings {
    pub visitor_new_title: &'static str,
    pub visitor_new_heading: &'static str,
    pub visitor_new_body: &'static str,
    pub visitor_new_footer: &'static str,
    pub visitor_wait_title: &'static str,
    pub visitor_wait_heading: &'static str,
    pub visitor_wait_body: &'static str,
    pub visitor_wait_footer: &'static str,
    pub badge_pending: &'static str,
    pub badge_approved: &'static str,
    pub admin_title: &'static str,
    pub admin_heading: &'static str,
    pub admin_th_user: &'static str,
    pub admin_th_domain: &'static str,
    pub admin_th_created: &'static str,
    pub admin_th_status: &'static str,
    pub admin_th_actions: &'static str,
    pub admin_th_ip: &'static str,
    pub admin_th_last_seen: &'static str,
    pub admin_th_ua: &'static str,
    pub admin_th_req_count: &'static str,
    pub admin_th_remark: &'static str,
    pub admin_empty: &'static str,
    pub admin_total: &'static str,
    pub admin_total_req: &'static str,
    pub btn_approve: &'static str,
    pub btn_revoke: &'static str,
    pub btn_delete: &'static str,
    pub confirm_revoke: &'static str,
    pub confirm_delete: &'static str,
    pub toast_approved: &'static str,
    pub toast_revoked: &'static str,
    pub toast_deleted: &'static str,
    pub toast_failed: &'static str,
    pub rate_limit_title: &'static str,
    pub rate_limit_body: &'static str,
    pub rate_limit_btn: &'static str,
    pub copy_btn: &'static str,
    pub copied: &'static str,
}

pub const EN: I18nStrings = I18nStrings {
    visitor_new_title: "Visitor Authentication",
    visitor_new_heading: "🔑 Visitor Authentication",
    visitor_new_body: "A visitor ID has been generated for you. Share it with the administrator to get access.",
    visitor_new_footer: "Refresh the page after approval.",
    visitor_wait_title: "Pending Approval",
    visitor_wait_heading: "⏳ Awaiting Admin Approval",
    visitor_wait_body: "Your visitor ID has been registered. Please wait for an administrator to approve your access.",
    visitor_wait_footer: "Refresh the page to check your status.",
    badge_pending: "Pending",
    badge_approved: "Approved",
    admin_title: "FAS Admin Panel",
    admin_heading: "🔑 FAS Visitor Management",
    admin_th_user: "User ID",
    admin_th_domain: "Domain",
    admin_th_created: "Created",
    admin_th_status: "Status",
    admin_th_actions: "Actions",
    admin_th_ip: "Last IP",
    admin_th_last_seen: "Last Seen",
    admin_th_ua: "User-Agent",
    admin_th_req_count: "Req#",
    admin_th_remark: "Remark",
    admin_empty: "No visitor records",
    admin_total: "Total visitors",
    admin_total_req: "Total requests",
    btn_approve: "Approve",
    btn_revoke: "Revoke",
    btn_delete: "Delete",
    confirm_revoke: "Revoke this user\u{2019}s access?",
    confirm_delete: "Permanently delete this record?",
    toast_approved: "✅ Approved",
    toast_revoked: "✅ Revoked",
    toast_deleted: "✅ Deleted",
    toast_failed: "❌ Operation failed",
    rate_limit_title: "Too Many Requests",
    rate_limit_body: "You are making requests too quickly. Please wait before trying again.",
    rate_limit_btn: "Retry in",
    copy_btn: "Copy",
    copied: "Copied!",
};

pub const ZH: I18nStrings = I18nStrings {
    visitor_new_title: "访客认证",
    visitor_new_heading: "🔑 访客认证",
    visitor_new_body: "系统已为您生成了一个访客 ID。请将此 ID 告知管理员以获取访问权限。",
    visitor_new_footer: "批准后刷新页面即可访问。",
    visitor_wait_title: "等待审批",
    visitor_wait_heading: "⏳ 等待管理员审批",
    visitor_wait_body: "您的访客 ID 已记录，请等待管理员批准访问权限。",
    visitor_wait_footer: "刷新页面以检查状态。",
    badge_pending: "待审批",
    badge_approved: "已许可",
    admin_title: "FAS 管理面板",
    admin_heading: "🔑 FAS 访客管理",
    admin_th_user: "用户 ID",
    admin_th_domain: "域名",
    admin_th_created: "创建时间",
    admin_th_status: "状态",
    admin_th_actions: "操作",
    admin_th_ip: "最后 IP",
    admin_th_last_seen: "最后访问",
    admin_th_ua: "User-Agent",
    admin_th_req_count: "请求#",
    admin_th_remark: "备注",
    admin_empty: "暂无访客记录",
    admin_total: "总访客",
    admin_total_req: "总请求",
    btn_approve: "许可",
    btn_revoke: "撤销",
    btn_delete: "删除",
    confirm_revoke: "确定撤销此用户的访问权限？",
    confirm_delete: "确定彻底删除此记录？",
    toast_approved: "✅ 已批准",
    toast_revoked: "✅ 已撤销",
    toast_deleted: "✅ 已删除",
    toast_failed: "❌ 操作失败",
    rate_limit_title: "请求过于频繁",
    rate_limit_body: "您的请求频率过高，请等待后再试。",
    rate_limit_btn: "等待",
    copy_btn: "复制",
    copied: "已复制！",
};

pub fn detect_locale(accept_language: &str) -> Locale {
    static RE_ZH_START: OnceLock<Regex> = OnceLock::new();
    static RE_ZH_SUB: OnceLock<Regex> = OnceLock::new();

    let re_start = RE_ZH_START.get_or_init(|| Regex::new(r"(?i)^zh\b").unwrap());
    let re_sub = RE_ZH_SUB.get_or_init(|| Regex::new(r"(?i)zh[-;]").unwrap());

    if re_start.is_match(accept_language) || re_sub.is_match(accept_language) {
        Locale::Zh
    } else {
        Locale::En
    }
}

pub fn t(locale: Locale) -> &'static I18nStrings {
    match locale {
        Locale::En => &EN,
        Locale::Zh => &ZH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_locale() {
        assert_eq!(detect_locale("zh-CN,zh;q=0.9"), Locale::Zh);
        assert_eq!(detect_locale("en-US,en;q=0.8,zh-TW;q=0.7"), Locale::Zh);
        assert_eq!(detect_locale("en-US,en;q=0.9"), Locale::En);
        assert_eq!(detect_locale(""), Locale::En);
        assert_eq!(detect_locale("ZH"), Locale::Zh);
        assert_eq!(detect_locale("en-GB,zh-CN;q=0.5"), Locale::Zh);
    }
}
