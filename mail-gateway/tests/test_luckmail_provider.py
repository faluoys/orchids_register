import json
from datetime import datetime, timezone

import httpx

from mail_gateway.providers.luckmail import LuckMailProvider


def test_luckmail_provider_acquires_purchased_inbox() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        assert request.url.path == "/api/v1/openapi/email/purchases/api-get"
        assert request.headers["X-API-Key"] == "AC-test-key"
        payload = json.loads(request.content.decode("utf-8"))
        assert payload["count"] == 1
        assert payload["tag_name"] == "orchids-ready"
        assert payload["mark_tag_name"] == "orchids-used"
        return httpx.Response(
            200,
            json={
                "code": 0,
                "message": "success",
                "data": [
                    {
                        "id": 1,
                        "email_address": "user1@outlook.com",
                        "token": "tok_abc123",
                        "project_name": "Orchids",
                        "tag_id": 2,
                        "tag_name": "orchids-used",
                    }
                ],
            },
        )

    provider = LuckMailProvider(
        base_url="https://mails.luckyous.com",
        api_key="AC-test-key",
        client=httpx.Client(
            transport=httpx.MockTransport(handler),
            base_url="https://mails.luckyous.com",
        ),
    )

    session = provider.acquire_inbox(
        project_code="orchids",
        metadata={"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
    )

    assert session.address == "user1@outlook.com"
    assert session.upstream_token == "tok_abc123"
    assert session.upstream_ref == "purchase:1"


def test_luckmail_provider_poll_code_falls_back_to_regex_extraction() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "GET"
        assert request.url.path == "/api/v1/openapi/email/token/tok_fallback/code"
        return httpx.Response(
            200,
            json={
                "code": 0,
                "message": "success",
                "data": {
                    "has_new_mail": True,
                    "verification_code": None,
                    "mail": {
                        "message_id": "msg-001",
                        "received_at": "2026-04-02T10:00:00Z",
                        "from": "noreply@orchids.com",
                        "subject": "Your Orchids verification code is 654321",
                        "body_text": "",
                        "body_html": "",
                    },
                },
            },
        )

    provider = LuckMailProvider(
        base_url="https://mails.luckyous.com",
        api_key="AC-test-key",
        client=httpx.Client(
            transport=httpx.MockTransport(handler),
            base_url="https://mails.luckyous.com",
        ),
    )

    result = provider.poll_code(
        upstream_token="tok_fallback",
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r"(\d{6})",
        after_ts=None,
    )

    assert result.status == "success"
    assert result.code == "654321"
    assert result.message_id == "msg-001"
    assert result.received_at == "2026-04-02T10:00:00Z"
    assert result.summary["from"] == "noreply@orchids.com"


def test_luckmail_provider_poll_code_with_after_ts_ignores_old_mail() -> None:
    calls = {"count": 0}

    def handler(request: httpx.Request) -> httpx.Response:
        calls["count"] += 1
        assert request.method == "GET"
        assert request.url.path == "/api/v1/openapi/email/token/tok_after_ts/code"
        if calls["count"] == 1:
            return httpx.Response(
                200,
                json={
                    "code": 0,
                    "message": "success",
                    "data": {
                        "has_new_mail": True,
                        "verification_code": "111111",
                        "mail": {
                            "message_id": "msg-old",
                            "received_at": "2026-04-02T10:00:00Z",
                            "from": "noreply@orchids.com",
                            "subject": "Old code",
                            "body_text": "",
                            "body_html": "",
                        },
                    },
                },
            )
        return httpx.Response(
            200,
            json={
                "code": 0,
                "message": "success",
                "data": {
                    "has_new_mail": True,
                    "verification_code": "222222",
                    "mail": {
                        "message_id": "msg-new",
                        "received_at": "2026-04-02T10:10:00Z",
                        "from": "noreply@orchids.com",
                        "subject": "New code",
                        "body_text": "",
                        "body_html": "",
                    },
                },
            },
        )

    provider = LuckMailProvider(
        base_url="https://mails.luckyous.com",
        api_key="AC-test-key",
        client=httpx.Client(
            transport=httpx.MockTransport(handler),
            base_url="https://mails.luckyous.com",
        ),
    )
    after_ts = int(datetime(2026, 4, 2, 10, 5, 0, tzinfo=timezone.utc).timestamp() * 1000)

    result = provider.poll_code(
        upstream_token="tok_after_ts",
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r"(\d{6})",
        after_ts=after_ts,
    )

    assert calls["count"] >= 2
    assert result.status == "success"
    assert result.code == "222222"
    assert result.message_id == "msg-new"


def test_luckmail_provider_acquire_inbox_raises_runtime_error_for_malformed_success_payload() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        assert request.url.path == "/api/v1/openapi/email/purchases/api-get"
        return httpx.Response(
            200,
            json={
                "code": 0,
                "message": "success",
                "data": [
                    {
                        "id": 1,
                        "email_address": "user1@outlook.com",
                    }
                ],
            },
        )

    provider = LuckMailProvider(
        base_url="https://mails.luckyous.com",
        api_key="AC-test-key",
        client=httpx.Client(
            transport=httpx.MockTransport(handler),
            base_url="https://mails.luckyous.com",
        ),
    )

    try:
        provider.acquire_inbox(
            project_code="orchids",
            metadata={"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        )
    except RuntimeError as exc:
        assert "LuckMail acquire failed" in str(exc)
    else:
        raise AssertionError("expected RuntimeError for malformed success payload")


def test_luckmail_provider_acquire_inbox_raises_runtime_error_for_invalid_values() -> None:
    invalid_records = [
        {"id": 1, "email_address": "", "token": "tok_abc123"},
        {"id": 1, "email_address": "user1@outlook.com", "token": ""},
        {"id": None, "email_address": "user1@outlook.com", "token": "tok_abc123"},
    ]

    for record in invalid_records:
        def handler(request: httpx.Request, rec: dict[str, object] = record) -> httpx.Response:
            assert request.method == "POST"
            assert request.url.path == "/api/v1/openapi/email/purchases/api-get"
            return httpx.Response(
                200,
                json={
                    "code": 0,
                    "message": "success",
                    "data": [rec],
                },
            )

        provider = LuckMailProvider(
            base_url="https://mails.luckyous.com",
            api_key="AC-test-key",
            client=httpx.Client(
                transport=httpx.MockTransport(handler),
                base_url="https://mails.luckyous.com",
            ),
        )

        try:
            provider.acquire_inbox(
                project_code="orchids",
                metadata={"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
            )
        except RuntimeError as exc:
            assert "LuckMail acquire failed" in str(exc)
        else:
            raise AssertionError("expected RuntimeError for invalid success values")
