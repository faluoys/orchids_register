import httpx
from fastapi.testclient import TestClient

from mail_gateway.app import create_app
from mail_gateway.config import Settings
from mail_gateway.services.session_service import SessionService


def _make_client() -> TestClient:
    settings = Settings(
        host="127.0.0.1",
        port=8081,
        database_path=":memory:",
        luckmail_base_url="https://mails.luckyous.com",
        luckmail_api_key="AC-test-key",
    )
    return TestClient(create_app(settings=settings, testing=True))


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


def test_acquire_with_unsupported_params_returns_400() -> None:
    client = _make_client()

    response = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "domain": "outlook.com",
            "prefix": "orchids",
            "quantity": 2,
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )

    assert response.status_code == 400
    assert response.json() == {"detail": "phase 1 does not support domain, prefix, or quantity overrides"}


def test_acquire_http_error_returns_502(monkeypatch) -> None:
    client = _make_client()

    def fake_acquire(self, provider_name: str, project_code: str | None, metadata: dict[str, str]):
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
    assert response.json() == {"detail": "provider error"}


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


def test_release_missing_session_returns_404() -> None:
    client = _make_client()

    response = client.delete("/v1/inboxes/ses_missing")

    assert response.status_code == 404
    assert response.json() == {"detail": "session not found"}
