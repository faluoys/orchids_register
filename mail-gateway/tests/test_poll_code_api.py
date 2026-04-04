import asyncio
from types import SimpleNamespace

import httpx
from fastapi.testclient import TestClient

from mail_gateway.app import create_app
from mail_gateway.config import Settings
import mail_gateway.providers.luckmail as luckmail_module
from mail_gateway.services.code_poll_service import CodePollService
from mail_gateway.services.session_service import SessionService


class FakeLuckMailClient:
    def __init__(self, user) -> None:
        self.user = user


def _make_client() -> TestClient:
    settings = Settings(
        host="127.0.0.1",
        port=8081,
        database_path=":memory:",
        luckmail_base_url="https://mails.luckyous.com",
        luckmail_api_key="AC-test-key",
        yyds_base_url='https://maliapi.215.im/v1',
        yyds_api_key='AC-yyds-test-key',
        mail_chatgpt_uk_base_url='https://mail.chatgpt.org.uk',
        mail_chatgpt_uk_api_key='AC-mail-chatgpt-uk-test-key',
    )
    return TestClient(create_app(settings=settings, testing=True))


def _sdk_dispatch(sync_value_factory):
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        return sync_value_factory()

    async def _async_value():
        return sync_value_factory()

    return _async_value()


def test_acquire_then_poll_then_release_returns_unified_contract() -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )
    assert acquired.status_code == 200
    session_id = acquired.json()["session_id"]
    assert acquired.json()["address"] == "user1@outlook.com"

    polled = client.post(
        f"/v1/inboxes/{session_id}/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "\\b(\\d{6})\\b",
            "after_ts": None,
        },
    )
    assert polled.status_code == 200
    assert polled.json()["status"] == "success"
    assert polled.json()["code"] == "482910"
    assert client.app.state.store.get_session(session_id).last_message_id == "msg_001"

    released = client.delete(f"/v1/inboxes/{session_id}")
    assert released.status_code == 204


def test_acquire_yyds_persistent_returns_unified_contract() -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "yyds_mail",
            "mode": "persistent",
            "project": "orchids",
            "domain": "example.com",
            "prefix": "orchids",
            "metadata": {},
        },
    )

    assert acquired.status_code == 200, acquired.text
    payload = acquired.json()
    assert payload['provider'] == 'yyds_mail'
    assert payload['mode'] == 'persistent'
    assert payload['address'] == 'orchids@example.com'
    assert payload['upstream_ref'] == 'inbox:ibox_stub'


def test_acquire_mail_chatgpt_uk_persistent_returns_unified_contract() -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "mail_chatgpt_uk",
            "mode": "persistent",
            "project": "orchids",
            "domain": "chatgpt.org.uk",
            "prefix": "orchids",
            "metadata": {},
        },
    )

    assert acquired.status_code == 200, acquired.text
    payload = acquired.json()
    assert payload['provider'] == 'mail_chatgpt_uk'
    assert payload['mode'] == 'persistent'
    assert payload['address'] == 'orchids@chatgpt.org.uk'
    assert payload['upstream_ref'] == 'inbox:mail_chatgpt_uk_stub'


def test_acquire_mail_chatgpt_uk_purchased_rejected() -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "mail_chatgpt_uk",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {},
        },
    )

    assert acquired.status_code == 400
    detail = acquired.json().get("detail", "")
    assert "mail_chatgpt_uk" in detail
    assert "persistent" in detail


def test_acquire_then_poll_then_release_works_with_sdk_auto_async_dispatch(monkeypatch) -> None:
    settings = Settings(
        host="127.0.0.1",
        port=8081,
        database_path=":memory:",
        luckmail_base_url="https://mails.luckyous.com",
        luckmail_api_key="AC-test-key",
        yyds_base_url='https://maliapi.215.im/v1',
        yyds_api_key='AC-yyds-test-key',
        mail_chatgpt_uk_base_url='https://mail.chatgpt.org.uk',
        mail_chatgpt_uk_api_key='AC-mail-chatgpt-uk-test-key',
    )

    class FakeUser:
        def api_get_purchases(self, count: int, tag_name: str | None = None, mark_tag_name: str | None = None):
            return _sdk_dispatch(
                lambda: [
                    SimpleNamespace(
                        id=1,
                        email_address="user1@outlook.com",
                        token="tok_abc123",
                    )
                ]
            )

        def get_token_code(self, token: str):
            return _sdk_dispatch(
                lambda: SimpleNamespace(
                    has_new_mail=True,
                    verification_code="654321",
                    mail={
                        "message_id": "msg-001",
                        "received_at": "2026-04-02T10:00:00Z",
                        "from": "noreply@orchids.com",
                        "subject": "Your Orchids verification code is 654321",
                        "body_text": "",
                        "body_html": "",
                    },
                )
            )

        def get_token_mail_detail(self, token: str, message_id: str):
            return _sdk_dispatch(
                lambda: SimpleNamespace(
                    message_id=message_id,
                    from_addr="noreply@orchids.com",
                    subject="Your Orchids verification code is 654321",
                    body_text="Your Orchids verification code is 654321",
                    body_html="",
                    received_at="2026-04-02T10:00:00Z",
                    verification_code="654321",
                )
            )

    monkeypatch.setattr(
        luckmail_module,
        "create_luckmail_client",
        lambda base_url, api_key: FakeLuckMailClient(FakeUser()),
    )

    client = TestClient(create_app(settings=settings, testing=False), raise_server_exceptions=False)

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {},
        },
    )
    assert acquired.status_code == 200, acquired.text

    session_id = acquired.json()["session_id"]
    polled = client.post(
        f"/v1/inboxes/{session_id}/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "\\b(\\d{6})\\b",
            "after_ts": None,
        },
    )
    assert polled.status_code == 200, polled.text
    assert polled.json()["status"] == "success"
    assert polled.json()["code"] == "654321"

    released = client.delete(f"/v1/inboxes/{session_id}")
    assert released.status_code == 204


def test_acquire_with_unsupported_params_returns_400() -> None:
    client = _make_client()

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "prefix": "orchids",
            "quantity": 2,
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )

    assert response.status_code == 400
    assert response.json() == {"detail": "phase 1 does not support prefix or quantity overrides"}


def test_acquire_with_domain_passes_through_to_session_service(monkeypatch) -> None:
    client = _make_client()
    captured: dict[str, object] = {}

    def fake_acquire(
        self,
        provider_name: str,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ):
        captured.update(
            {
                'provider_name': provider_name,
                'project_code': project_code,
                'domain': domain,
                'metadata': metadata,
            }
        )
        return SimpleNamespace(
            session_id='ses_domain',
            provider=provider_name,
            mode='purchased',
            address='user2@hotmail.com',
            upstream_token='tok_hotmail',
            upstream_ref='purchase:2',
            expires_at=None,
        )

    monkeypatch.setattr(SessionService, 'acquire', fake_acquire)

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "domain": "hotmail.com",
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )

    assert response.status_code == 200
    assert captured == {
        'provider_name': 'luckmail',
        'project_code': 'orchids',
        'domain': 'hotmail.com',
        'metadata': {'tag_name': 'orchids-ready', 'mark_tag_name': 'orchids-used'},
    }
    assert response.json()['address'] == 'user2@hotmail.com'


def test_acquire_yyds_prefix_is_forwarded_in_metadata(monkeypatch) -> None:
    client = _make_client()
    captured: dict[str, object] = {}

    def fake_acquire(
        self,
        provider_name: str,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ):
        captured.update(
            {
                'provider_name': provider_name,
                'project_code': project_code,
                'domain': domain,
                'metadata': metadata,
            }
        )
        return SimpleNamespace(
            session_id='ses_yyds',
            provider=provider_name,
            mode='persistent',
            address='orchids@example.com',
            upstream_token='orchids@example.com',
            upstream_ref='inbox:ibox_123',
            expires_at=None,
        )

    monkeypatch.setattr(SessionService, 'acquire', fake_acquire)

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "yyds_mail",
            "mode": "persistent",
            "project": "orchids",
            "domain": "example.com",
            "prefix": "orchids",
            "metadata": {},
        },
    )

    assert response.status_code == 200, response.text
    assert captured == {
        'provider_name': 'yyds_mail',
        'project_code': 'orchids',
        'domain': 'example.com',
        'metadata': {'prefix': 'orchids'},
    }
    assert response.json()['address'] == 'orchids@example.com'


def test_acquire_http_error_returns_502(monkeypatch) -> None:
    client = _make_client()

    def fake_acquire(self, provider_name: str, project_code: str | None, domain: str | None, metadata: dict[str, str]):
        raise httpx.HTTPError("boom")

    monkeypatch.setattr(SessionService, "acquire", fake_acquire)

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )

    assert response.status_code == 502
    assert response.json() == {"detail": "boom"}


def test_acquire_runtime_error_returns_502_with_upstream_detail(monkeypatch) -> None:
    client = _make_client()

    def fake_acquire(self, provider_name: str, project_code: str | None, domain: str | None, metadata: dict[str, str]):
        raise RuntimeError("YYDS request failed: API key lacks write permission for this operation")

    monkeypatch.setattr(SessionService, "acquire", fake_acquire)

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "yyds_mail",
            "mode": "persistent",
            "project": "orchids",
            "metadata": {},
        },
    )

    assert response.status_code == 502
    assert response.json() == {"detail": "YYDS request failed: API key lacks write permission for this operation"}


def test_poll_code_unknown_session_returns_404() -> None:
    client = _make_client()

    response = client.post(
        "/v1/inboxes/ses_missing/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "\\b(\\d{6})\\b",
            "after_ts": None,
        },
    )

    assert response.status_code == 404
    assert response.json() == {"detail": "session not found"}


def test_poll_code_invalid_pattern_returns_400() -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {},
        },
    )
    session_id = acquired.json()["session_id"]

    response = client.post(
        f"/v1/inboxes/{session_id}/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "(",
            "after_ts": None,
        },
    )

    assert response.status_code == 400
    assert response.json() == {"detail": "invalid code_pattern"}


def test_poll_code_runtime_error_returns_502(monkeypatch) -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {},
        },
    )
    session_id = acquired.json()["session_id"]

    def fake_poll_code(
        self,
        session_id: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ):
        raise RuntimeError("provider poll exploded")

    monkeypatch.setattr(CodePollService, "poll_code", fake_poll_code)

    response = client.post(
        f"/v1/inboxes/{session_id}/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "\\b(\\d{6})\\b",
            "after_ts": None,
        },
    )

    assert response.status_code == 502
    assert response.json() == {"detail": "provider poll exploded"}


def test_release_missing_session_returns_404() -> None:
    client = _make_client()

    response = client.delete("/v1/inboxes/ses_missing")

    assert response.status_code == 404
    assert response.json() == {"detail": "session not found"}


def test_release_runtime_error_returns_502(monkeypatch) -> None:
    client = _make_client()

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {},
        },
    )
    session_id = acquired.json()["session_id"]

    def fake_release(self, session_id: str) -> None:
        raise RuntimeError("release failed upstream")

    monkeypatch.setattr(SessionService, "release", fake_release)

    response = client.delete(f"/v1/inboxes/{session_id}")

    assert response.status_code == 502
    assert response.json() == {"detail": "release failed upstream"}

