pub const CLERK_BASE: &str = "https://clerk.orchids.app";
pub const API_VERSION: &str = "2025-11-10";
pub const JS_VERSION: &str = "5.125.3";

pub const TEMPMAIL_BASE: &str = "https://api.tempmail.lol/v2";

pub const DEFAULT_TEST_USE_CAPMONSTER: bool = true;
pub const DEFAULT_TEST_CAPMONSTER_WEBSITE_KEY: &str = "0x4AAAAAAAWXJGBD7bONzLBd";
pub const DEFAULT_TEST_POLL_INTERVAL: f64 = 2.0;
pub const DEFAULT_TEST_DEBUG_TEMPMAIL: bool = true;

pub fn clerk_params() -> [(&'static str, &'static str); 2] {
    [
        ("__clerk_api_version", API_VERSION),
        ("_clerk_js_version", JS_VERSION),
    ]
}

pub fn user_agent() -> &'static str {
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"
}

/// 生成随机密码：12-16 位，包含大小写字母、数字和特殊字符
pub fn generate_random_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let upper = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let lower = b"abcdefghijklmnopqrstuvwxyz";
    let digits = b"0123456789";
    let special = b"!@#$%&*?";
    let all: Vec<u8> = [&upper[..], &lower[..], &digits[..], &special[..]].concat();

    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let mut next = || -> u64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let len = 12 + (next() % 5) as usize; // 12-16
    let mut password = Vec::with_capacity(len);

    // 保证至少包含每种字符各一个
    password.push(upper[(next() % upper.len() as u64) as usize]);
    password.push(lower[(next() % lower.len() as u64) as usize]);
    password.push(digits[(next() % digits.len() as u64) as usize]);
    password.push(special[(next() % special.len() as u64) as usize]);

    for _ in 4..len {
        password.push(all[(next() % all.len() as u64) as usize]);
    }

    // Fisher-Yates shuffle
    for i in (1..password.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        password.swap(i, j);
    }

    String::from_utf8(password).unwrap_or_else(|_| "Rn8!xK3@mP5q".to_string())
}
