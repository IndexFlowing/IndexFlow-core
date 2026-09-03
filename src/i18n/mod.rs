use axum::http::header::{ACCEPT_LANGUAGE, COOKIE};
use axum::http::HeaderMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Zh,
    En,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
        }
    }
}

#[allow(dead_code)]
pub struct Translations {
    // 导航与通用
    pub nav_dashboard: &'static str,
    pub nav_urls: &'static str,
    pub nav_settings: &'static str,
    pub nav_monitoring: &'static str,
    pub footer_text: &'static str,
    pub dry_run_badge: &'static str,
    pub monitoring_title: &'static str,
    pub monitoring_watched: &'static str,
    pub monitoring_candidates: &'static str,
    pub monitoring_add: &'static str,
    pub monitoring_remove: &'static str,
    pub monitoring_empty: &'static str,
    pub monitoring_sitemap: &'static str,
    pub monitoring_seo: &'static str,
    pub monitoring_submission: &'static str,
    pub monitoring_index_status: &'static str,

    // 控制台 Dashboard
    pub unconfigured_site: &'static str,
    pub unset: &'static str,
    pub btn_sync_sitemap: &'static str,
    pub btn_inspect_gsc: &'static str,
    pub btn_audit_seo: &'static str,
    pub btn_submit_all: &'static str,
    pub card_url_total: &'static str,
    pub card_url_total_desc: &'static str,
    pub card_google_indexed: &'static str,
    pub card_google_indexed_desc: &'static str,
    pub card_google_not_indexed: &'static str,
    pub card_google_not_indexed_desc: &'static str,
    pub card_seo_issues: &'static str,
    pub card_seo_issues_desc: &'static str,
    pub card_pending_submit: &'static str,
    pub card_pending_submit_desc: &'static str,
    pub table_recent_title: &'static str,
    pub table_view_all: &'static str,

    // 表格表头与状态
    pub th_url: &'static str,
    pub th_google_index: &'static str,
    pub th_seo_audit: &'static str,
    pub th_bing_submit: &'static str,
    pub th_google_submit: &'static str,
    pub th_actions: &'static str,
    pub status_indexed: &'static str,
    pub status_not_indexed: &'static str,
    pub status_uninspected: &'static str,
    pub status_seo_pass: &'static str,
    pub status_seo_pending: &'static str,
    pub status_submitted: &'static str,

    // 站点设置
    pub settings_title: &'static str,
    pub settings_desc: &'static str,
    pub label_domain: &'static str,
    pub label_sitemap: &'static str,
    pub label_bing_key: &'static str,
    pub label_google_sa: &'static str,
    pub btn_save_settings: &'static str,

    // 登录 / 初始化向导
    pub login_title: &'static str,
    pub setup_title: &'static str,
    pub login_desc: &'static str,
    pub setup_desc: &'static str,
    pub label_username: &'static str,
    pub label_password: &'static str,
    pub btn_login: &'static str,
    pub btn_setup: &'static str,
    pub auth_hero_title: &'static str,
    pub auth_hero_subtitle: &'static str,
}

pub const ZH: Translations = Translations {
    nav_dashboard: "控制台",
    nav_urls: "URL 列表",
    nav_settings: "站点设置",
    nav_monitoring: "收录监测",
    footer_text: "IndexFlow · 开源搜索引擎收录与技术 SEO 基础设施",
    dry_run_badge: "Dry-Run 演练模式 (安全日志拦截)",
    monitoring_title: "收录监测",
    monitoring_watched: "监测中的 URL",
    monitoring_candidates: "从已解析 URL 中添加",
    monitoring_add: "加入监测",
    monitoring_remove: "移出监测",
    monitoring_empty: "暂无监测中的 URL",
    monitoring_sitemap: "Sitemap",
    monitoring_seo: "SEO 检测",
    monitoring_submission: "提交记录",
    monitoring_index_status: "收录状态变化",

    unconfigured_site: "未配置站点",
    unset: "未设置",
    btn_sync_sitemap: "同步 Sitemap",
    btn_inspect_gsc: "GSC 收录检测",
    btn_audit_seo: "运行 SEO 质检",
    btn_submit_all: "一键全引擎提交",
    card_url_total: "URL 总数",
    card_url_total_desc: "Sitemap 发现总数",
    card_google_indexed: "Google 已收录",
    card_google_indexed_desc: "Search Console 确认",
    card_google_not_indexed: "Google 未收录",
    card_google_not_indexed_desc: "待抓取 / 抓取未编入",
    card_seo_issues: "SEO 拦截项",
    card_seo_issues_desc: "404 / noindex / 错标",
    card_pending_submit: "待提交引擎",
    card_pending_submit_desc: "Bing / Google 队列",
    table_recent_title: "最近 URL 状态清单",
    table_view_all: "查看完整列表",

    th_url: "URL 路径与标题",
    th_google_index: "Google 收录",
    th_seo_audit: "SEO 质检",
    th_bing_submit: "Bing IndexNow",
    th_google_submit: "Google API",
    th_actions: "单项操作",
    status_indexed: "已收录",
    status_not_indexed: "未收录",
    status_uninspected: "未检测",
    status_seo_pass: "正常 (200)",
    status_seo_pending: "待检测",
    status_submitted: "已推送",

    settings_title: "站点与凭证设置",
    settings_desc: "配置要管理的网站域名、Sitemap 路径及搜索引擎官方提交授权凭据。",
    label_domain: "网站域名",
    label_sitemap: "Sitemap 地址",
    label_bing_key: "Bing IndexNow API Key",
    label_google_sa: "Google Service Account 密钥 (JSON 字符串)",
    btn_save_settings: "保存站点配置",

    login_title: "登录控制台",
    setup_title: "初始化系统管理员",
    login_desc: "请输入管理员账号与密码登录控制台",
    setup_desc: "首次部署：请设置你的最高管理员账号与密码",
    label_username: "管理员账号",
    label_password: "登录密码",
    btn_login: "立即登录控制台",
    btn_setup: "完成初始化并进入控制台",
    auth_hero_title: "掌握全站索引收录生命周期",
    auth_hero_subtitle: "将流量主动权还给开发者",
};

pub const EN: Translations = Translations {
    nav_dashboard: "Dashboard",
    nav_urls: "URLs",
    nav_settings: "Settings",
    nav_monitoring: "Indexing Watch",
    footer_text: "IndexFlow · Open-Core Search Index & Technical SEO Infrastructure",
    dry_run_badge: "Dry-Run Mode (Safe Logs Only)",
    monitoring_title: "Indexing Watch",
    monitoring_watched: "Watched URLs",
    monitoring_candidates: "Add from parsed URLs",
    monitoring_add: "Add to watch",
    monitoring_remove: "Remove from watch",
    monitoring_empty: "No URLs are being watched",
    monitoring_sitemap: "Sitemap",
    monitoring_seo: "SEO check",
    monitoring_submission: "Submission",
    monitoring_index_status: "Index status change",

    unconfigured_site: "Unconfigured Site",
    unset: "Unset",
    btn_sync_sitemap: "Sync Sitemap",
    btn_inspect_gsc: "Inspect GSC",
    btn_audit_seo: "Run SEO Audit",
    btn_submit_all: "Submit to All Engines",
    card_url_total: "Total URLs",
    card_url_total_desc: "Discovered in Sitemaps",
    card_google_indexed: "Google Indexed",
    card_google_indexed_desc: "Search Console Confirmed",
    card_google_not_indexed: "Not Indexed",
    card_google_not_indexed_desc: "Discovered / Crawled Not Indexed",
    card_seo_issues: "SEO Issues",
    card_seo_issues_desc: "404 / noindex / mismatch",
    card_pending_submit: "Pending Submit",
    card_pending_submit_desc: "Bing / Google Queue",
    table_recent_title: "Recent URL Statuses",
    table_view_all: "View All URLs",

    th_url: "URL & Page Title",
    th_google_index: "Google Index",
    th_seo_audit: "SEO Health",
    th_bing_submit: "Bing IndexNow",
    th_google_submit: "Google API",
    th_actions: "Actions",
    status_indexed: "Indexed",
    status_not_indexed: "Not Indexed",
    status_uninspected: "Uninspected",
    status_seo_pass: "Passed (200)",
    status_seo_pending: "Pending",
    status_submitted: "Submitted",

    settings_title: "Site & Engine Credentials",
    settings_desc:
        "Configure target domain, sitemap URL, and search engine API authorization keys.",
    label_domain: "Domain Name",
    label_sitemap: "Sitemap URL",
    label_bing_key: "Bing IndexNow API Key",
    label_google_sa: "Google Service Account Key (JSON String)",
    btn_save_settings: "Save Site Configuration",

    login_title: "Admin Login",
    setup_title: "Initialize Admin User",
    login_desc: "Sign in with your administrator credentials",
    setup_desc: "First setup: Create your super administrator account",
    label_username: "Username",
    label_password: "Password",
    btn_login: "Sign In",
    btn_setup: "Complete Setup & Launch",
    auth_hero_title: "Full Control Over Your Indexing Pipeline",
    auth_hero_subtitle: "Take back your organic search traffic",
};

pub fn get_translations(lang: Language) -> &'static Translations {
    match lang {
        Language::Zh => &ZH,
        Language::En => &EN,
    }
}

pub fn detect_language(
    headers: &HeaderMap,
    query_lang: Option<&str>,
) -> (Language, Option<String>) {
    if let Some(ql) = query_lang {
        let l = ql.to_ascii_lowercase();
        if l.starts_with("en") {
            return (
                Language::En,
                Some("if_lang=en; Path=/; Max-Age=31536000; SameSite=Lax".into()),
            );
        } else if l.starts_with("zh") {
            return (
                Language::Zh,
                Some("if_lang=zh; Path=/; Max-Age=31536000; SameSite=Lax".into()),
            );
        }
    }

    if let Some(cookie_str) = headers.get(COOKIE).and_then(|v| v.to_str().ok()) {
        for cookie in cookie_str.split(';') {
            let mut parts = cookie.trim().splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                if k == "if_lang" {
                    if v.starts_with("en") {
                        return (Language::En, None);
                    }
                    if v.starts_with("zh") {
                        return (Language::Zh, None);
                    }
                }
            }
        }
    }

    if let Some(accept_lang) = headers.get(ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()) {
        let first = accept_lang
            .split(',')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if first.starts_with("zh") {
            return (Language::Zh, None);
        }
    }

    (Language::En, None)
}
