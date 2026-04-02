use clap::Parser;
use orchids_core::cli::Args;
use orchids_core::inbox_gateway::{
    gateway_poll_http_timeout_secs, AcquireInboxResponse, GatewaySettings, PollCodeResponse,
};

#[test]
fn gateway_settings_only_enable_gateway_mode_when_url_and_provider_exist() {
    let enabled_args = Args::parse_from([
        "orchids-auto-register",
        "--mail-mode",
        "gateway",
        "--mail-gateway-base-url",
        "http://127.0.0.1:8081",
        "--mail-provider",
        "luckmail",
    ]);
    assert!(GatewaySettings::from_args(&enabled_args).enabled());

    let missing_url_args = Args::parse_from([
        "orchids-auto-register",
        "--mail-mode",
        "gateway",
        "--mail-provider",
        "luckmail",
    ]);
    assert!(!GatewaySettings::from_args(&missing_url_args).enabled());

    let missing_provider_args = Args::parse_from([
        "orchids-auto-register",
        "--mail-mode",
        "gateway",
        "--mail-gateway-base-url",
        "http://127.0.0.1:8081",
        "--mail-provider",
        "",
    ]);
    assert!(!GatewaySettings::from_args(&missing_provider_args).enabled());

    let manual_args = Args::parse_from([
        "orchids-auto-register",
        "--mail-mode",
        "manual",
        "--mail-gateway-base-url",
        "http://127.0.0.1:8081",
        "--mail-provider",
        "luckmail",
    ]);
    assert!(!GatewaySettings::from_args(&manual_args).enabled());
}

#[test]
fn parse_gateway_responses_from_json() {
    let acquire: AcquireInboxResponse = serde_json::from_str(
        r#"{
            "session_id": "ses_123",
            "address": "user@example.com",
            "provider": "luckmail",
            "mode": "purchased",
            "expires_at": "2026-04-02T16:10:20Z",
            "upstream_ref": "purchase:1"
        }"#,
    )
    .expect("acquire response should deserialize");
    assert_eq!(acquire.session_id, "ses_123");
    assert_eq!(acquire.address, "user@example.com");
    assert_eq!(acquire.provider, "luckmail");
    assert_eq!(acquire.mode, "purchased");
    assert_eq!(acquire.expires_at.as_deref(), Some("2026-04-02T16:10:20Z"));
    assert_eq!(acquire.upstream_ref, "purchase:1");

    let poll: PollCodeResponse = serde_json::from_str(
        r#"{
            "status": "success",
            "code": "482910",
            "message_id": "msg_001",
            "received_at": "2026-04-02T16:10:20Z",
            "summary": {
                "from": "info@orchids.app",
                "subject": "Your verification code"
            }
        }"#,
    )
    .expect("poll response should deserialize");
    assert_eq!(poll.status, "success");
    assert_eq!(poll.code.as_deref(), Some("482910"));
    assert_eq!(poll.message_id.as_deref(), Some("msg_001"));
    assert_eq!(poll.received_at.as_deref(), Some("2026-04-02T16:10:20Z"));
    assert_eq!(poll.summary.get("from").map(String::as_str), Some("info@orchids.app"));
    assert_eq!(
        poll.summary.get("subject").map(String::as_str),
        Some("Your verification code")
    );
}

#[test]
fn gateway_poll_timeout_uses_poll_timeout_floor_with_buffer() {
    assert_eq!(gateway_poll_http_timeout_secs(30, 180), 185);
    assert_eq!(gateway_poll_http_timeout_secs(300, 180), 300);
}

#[test]
fn freemail_legacy_flag_defaults_to_inactive() {
    let args = Args::parse_from(["orchids-auto-register"]);
    assert!(!args.use_freemail);
}
