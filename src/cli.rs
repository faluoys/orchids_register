use clap::{ArgAction, Parser};

use crate::constants::{
    DEFAULT_TEST_CAPMONSTER_WEBSITE_KEY, DEFAULT_TEST_DEBUG_TEMPMAIL,
    DEFAULT_TEST_POLL_INTERVAL, DEFAULT_TEST_USE_CAPMONSTER,
};

#[derive(Debug, Clone, Parser)]
#[command(about = "通过 Orchids/Clerk 接口注册账号")]
pub struct Args {
    #[arg(long, default_value = None, help = "注册邮箱（不使用临时邮箱时必填）")]
    pub email: Option<String>,

    #[arg(long, default_value = None, help = "注册密码（不传则自动生成随机密码）")]
    pub password: Option<String>,

    #[arg(long = "captcha-token", default_value = None, help = "Cloudflare Turnstile/captcha 实时 token")]
    pub captcha_token: Option<String>,

    #[arg(long = "use-capmonster", action = ArgAction::SetTrue, default_value_t = DEFAULT_TEST_USE_CAPMONSTER, help = "使用本地打码 API 自动求解 Turnstile")]
    pub use_capmonster: bool,

    #[arg(long = "captcha-api-url", default_value = "http://127.0.0.1:5000", help = "本地打码 API 地址")]
    pub captcha_api_url: String,

    #[arg(long = "captcha-timeout", default_value_t = 180, help = "打码轮询超时秒数，默认 180")]
    pub captcha_timeout: i64,

    #[arg(long = "captcha-poll-interval", default_value_t = 3.0, help = "打码轮询间隔秒数，默认 3")]
    pub captcha_poll_interval: f64,

    #[arg(long = "captcha-website-url", default_value = "https://accounts.orchids.app/", help = "打码任务 websiteURL")]
    pub captcha_website_url: String,

    #[arg(long = "captcha-website-key", default_value = DEFAULT_TEST_CAPMONSTER_WEBSITE_KEY, help = "打码任务 websiteKey（Turnstile sitekey）")]
    pub captcha_website_key: String,


    #[arg(long = "email-code", default_value = None, help = "邮箱验证码；不传则只执行到发送验证码")]
    pub email_code: Option<String>,

    #[arg(long, default_value = "zh-CN", help = "语言，默认 zh-CN")]
    pub locale: String,

    #[arg(long, default_value_t = 30, help = "请求超时秒数，默认 30")]
    pub timeout: i64,

    #[arg(long = "mail-mode", default_value = "gateway", help = "邮箱模式：gateway 或 manual，默认 gateway")]
    pub mail_mode: String,

    #[arg(long = "mail-gateway-base-url", default_value = None, help = "mail-gateway 基础 URL")]
    pub mail_gateway_base_url: Option<String>,

    #[arg(long = "mail-gateway-api-key", default_value = None, help = "mail-gateway API Key")]
    pub mail_gateway_api_key: Option<String>,

    #[arg(long = "mail-provider", default_value = "luckmail", help = "邮箱 provider，默认 luckmail")]
    pub mail_provider: String,

    #[arg(long = "mail-provider-mode", default_value = "purchased", help = "邮箱 provider mode，默认 purchased")]
    pub mail_provider_mode: String,

    #[arg(long = "mail-project-code", default_value = None, help = "mail-gateway project code")]
    pub mail_project_code: Option<String>,

    #[arg(long = "mail-domain", default_value = None, help = "mail-gateway 指定域名")]
    pub mail_domain: Option<String>,

    #[arg(long = "use-freemail", action = ArgAction::SetTrue, default_value_t = false, help = "legacy/inactive：旧 freemail 自动建箱/拉码开关，当前主流程不再使用")]
    pub use_freemail: bool,

    #[arg(long = "freemail-base-url", default_value = None, help = "legacy/inactive：旧 freemail API 基础 URL，当前主流程不消费")]
    pub freemail_base_url: Option<String>,

    #[arg(long = "freemail-admin-token", default_value = None, help = "legacy/inactive：旧 freemail 管理员令牌（JWT_TOKEN），当前主流程不消费")]
    pub freemail_admin_token: Option<String>,

    #[arg(long = "freemail-domain-index", default_value_t = 0, help = "legacy/inactive：旧 freemail 域名索引，当前主流程不消费")]
    pub freemail_domain_index: i32,

    #[arg(long = "poll-timeout", default_value_t = 180, help = "自动轮询验证码超时秒数，默认 180")]
    pub poll_timeout: i64,

    #[arg(long = "poll-interval", default_value_t = DEFAULT_TEST_POLL_INTERVAL, help = "轮询间隔秒数")]
    pub poll_interval: f64,

    #[arg(long = "code-pattern", default_value = "\\b(\\d{6})\\b", help = "验证码提取正则，默认提取 6 位数字")]
    pub code_pattern: String,

    #[arg(long = "debug-email", action = ArgAction::SetTrue, default_value_t = DEFAULT_TEST_DEBUG_TEMPMAIL, help = "输出邮箱轮询调试信息")]
    pub debug_email: bool,

    #[arg(long = "result-json", default_value = "register_result.json", help = "注册结果输出 JSON 文件路径")]
    pub result_json: String,

    #[arg(long = "test-desktop-session", action = ArgAction::SetTrue, default_value_t = true, help = "注册后测试桌面应用会话可用性")]
    pub test_desktop_session: bool,

    #[arg(long, default_value = None, help = "代理地址，如 http://host:port 或 socks5://host:port")]
    pub proxy: Option<String>,

    #[arg(long = "use-proxy-pool", action = ArgAction::SetTrue, default_value_t = false, help = "使用代理池，每个注册线程使用一个代理")]
    pub use_proxy_pool: bool,

    #[arg(long = "proxy-pool-api", default_value = "https://api.douyadaili.com/proxy/?service=GetUnl&authkey=1KB6xBwGlITDeICSw6BI&num=10&lifetime=1&prot=0&format=txt&cstmfmt=%7Bip%7D%7C%7Bport%7D&separator=%5Cr%5Cn&distinct=1&detail=0&portlen=0", help = "代理池 API 地址")]
    pub proxy_pool_api: String,
}

pub fn parse_args() -> Args {
    Args::parse()
}

