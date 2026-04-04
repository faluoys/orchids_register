import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import httpx
import pytest

sys.path.append(str(Path(__file__).resolve().parents[1]))

import mail_gateway.providers.mail_chatgpt_uk as mail_chatgpt_uk_module
from mail_gateway.providers.mail_chatgpt_uk import MailChatGPTUKProvider


def _make_provider(handler) -> MailChatGPTUKProvider:
    client = httpx.Client(
        transport=httpx.MockTransport(handler),
        base_url='https://mail.chatgpt.org.uk',
    )
    return MailChatGPTUKProvider(
        base_url='https://mail.chatgpt.org.uk',
        api_key='AC-chatgpt-uk-test-key',
        client=client,
    )


def test_chatgpt_uk_acquire_uses_get_without_prefix_or_domain() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == 'GET'
        assert request.url.path == '/api/generate-email'
        assert request.headers['X-API-Key'] == 'AC-chatgpt-uk-test-key'
        assert request.content == b''
        return httpx.Response(
            200,
            json={'data': {'email': 'orchids@example.com'}},
        )

    provider = _make_provider(handler)

    acquired = provider.acquire_inbox(
        project_code=None,
        domain=None,
        metadata={},
    )

    assert acquired.address == 'orchids@example.com'
    assert acquired.upstream_token == 'orchids@example.com'
    assert acquired.upstream_ref == 'inbox:orchids@example.com'
    assert acquired.expires_at is None


def test_chatgpt_uk_acquire_uses_post_when_prefix_or_domain_present() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == 'POST'
        assert request.url.path == '/api/generate-email'
        assert request.headers['X-API-Key'] == 'AC-chatgpt-uk-test-key'
        assert json.loads(request.content.decode('utf-8')) == {
            'prefix': 'orchids',
            'domain': 'example.com',
        }
        return httpx.Response(
            200,
            json={'data': {'email': 'orchids@example.com'}},
        )

    provider = _make_provider(handler)

    acquired = provider.acquire_inbox(
        project_code='orchids',
        domain='example.com',
        metadata={'prefix': 'orchids'},
    )

    assert acquired.address == 'orchids@example.com'
    assert acquired.upstream_token == 'orchids@example.com'
    assert acquired.upstream_ref == 'inbox:orchids@example.com'
    assert acquired.expires_at is None


def test_chatgpt_uk_acquire_raises_when_email_missing() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={'data': {}})

    provider = _make_provider(handler)

    with pytest.raises(RuntimeError, match='data.email'):
        provider.acquire_inbox(
            project_code=None,
            domain=None,
            metadata={},
        )


def test_chatgpt_uk_poll_code_success_after_ts(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)
    after_ts = int(datetime(2026, 4, 3, 10, 5, 0, tzinfo=timezone.utc).timestamp() * 1000)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.headers['X-API-Key'] == 'AC-chatgpt-uk-test-key'
        if request.url.path == '/api/emails':
            assert request.method == 'GET'
            assert request.url.params['email'] == 'orchids@example.com'
            return httpx.Response(
                200,
                json={
                    'data': [
                        {
                            'id': 'msg_old',
                            'createdAt': '2026-04-03T10:00:00Z',
                        },
                        {
                            'id': 'msg_new',
                            'createdAt': '2026-04-03T10:10:00Z',
                        },
                    ],
                },
            )
        if request.url.path == '/api/email/msg_new':
            assert request.method == 'GET'
            return httpx.Response(
                200,
                json={
                    'data': {
                        'id': 'msg_new',
                        'subject': 'Your Orchids verification code',
                        'text': 'Your verification code is 654321',
                        'html': '<p>Your verification code is <strong>654321</strong></p>',
                        'createdAt': '2026-04-03T10:10:00Z',
                        'from': 'noreply@orchids.app',
                    },
                },
            )
        raise AssertionError(f'unexpected request: {request.method} {request.url}')

    provider = _make_provider(handler)

    result = provider.poll_code(
        upstream_token='orchids@example.com',
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=after_ts,
    )

    assert result.status == 'success'
    assert result.code == '654321'
    assert result.message_id == 'msg_new'
    assert result.received_at == '2026-04-03T10:10:00Z'
    assert result.summary == {
        'from': 'noreply@orchids.app',
        'subject': 'Your Orchids verification code',
    }


def test_chatgpt_uk_poll_code_timeout(monkeypatch) -> None:
    clock = {'value': 0.0}

    def fake_time() -> float:
        clock['value'] += 0.6
        return clock['value']

    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'time', fake_time)
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == '/api/emails'
        return httpx.Response(200, json={'data': []})

    provider = _make_provider(handler)

    result = provider.poll_code(
        upstream_token='orchids@example.com',
        timeout_seconds=1,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=None,
    )

    assert result.status == 'timeout'
    assert result.code is None
    assert result.message_id is None
    assert result.received_at is None
    assert result.summary == {}


def test_chatgpt_uk_poll_code_raises_when_email_list_is_not_list(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == '/api/emails'
        return httpx.Response(200, json={'data': {'id': 'not-a-list'}})

    provider = _make_provider(handler)

    with pytest.raises(RuntimeError, match='/api/emails.*list'):
        provider.poll_code(
            upstream_token='orchids@example.com',
            timeout_seconds=1,
            interval_seconds=0.1,
            code_pattern=r'(\d{6})',
            after_ts=None,
        )


def test_chatgpt_uk_poll_code_raises_when_email_detail_is_not_dict(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == '/api/emails':
            return httpx.Response(
                200,
                json={
                    'data': [
                        {
                            'id': 'msg_invalid_detail',
                            'createdAt': '2026-04-03T11:00:00Z',
                        },
                    ],
                },
            )
        if request.url.path == '/api/email/msg_invalid_detail':
            return httpx.Response(200, json={'data': ['not-a-dict']})
        raise AssertionError(f'unexpected request: {request.method} {request.url}')

    provider = _make_provider(handler)

    with pytest.raises(RuntimeError, match='/api/email/msg_invalid_detail.*dict'):
        provider.poll_code(
            upstream_token='orchids@example.com',
            timeout_seconds=1,
            interval_seconds=0.1,
            code_pattern=r'(\d{6})',
            after_ts=None,
        )


def test_chatgpt_uk_poll_code_prefers_text_then_html_then_subject(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == '/api/emails':
            return httpx.Response(
                200,
                json={
                    'data': [
                        {
                            'id': 'msg_priority',
                            'createdAt': '2026-04-03T11:00:00Z',
                        },
                    ],
                },
            )
        if request.url.path == '/api/email/msg_priority':
            return httpx.Response(
                200,
                json={
                    'data': {
                        'id': 'msg_priority',
                        'subject': 'Subject code 333333',
                        'text': 'Text code 111111',
                        'html': '<p>HTML code <strong>222222</strong></p>',
                        'createdAt': '2026-04-03T11:00:00Z',
                        'from': 'noreply@orchids.app',
                    },
                },
            )
        raise AssertionError(f'unexpected request: {request.method} {request.url}')

    provider = _make_provider(handler)

    result = provider.poll_code(
        upstream_token='orchids@example.com',
        timeout_seconds=1,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=None,
    )

    assert result.status == 'success'
    assert result.code == '111111'


def test_chatgpt_uk_poll_code_after_ts_uses_message_created_at_when_detail_missing(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)
    after_ts = int(datetime(2026, 4, 3, 11, 0, 0, tzinfo=timezone.utc).timestamp() * 1000)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == '/api/emails':
            return httpx.Response(
                200,
                json={
                    'data': [
                        {
                            'id': 'msg_new',
                            'createdAt': '2026-04-03T11:10:00Z',
                        },
                    ],
                },
            )
        if request.url.path == '/api/email/msg_new':
            return httpx.Response(
                200,
                json={
                    'data': {
                        'id': 'msg_new',
                        'subject': 'Your Orchids verification code',
                        'text': 'Your verification code is 123456',
                        'html': '<p>Your verification code is <strong>123456</strong></p>',
                        'from': 'noreply@orchids.app',
                    },
                },
            )
        raise AssertionError(f'unexpected request: {request.method} {request.url}')

    provider = _make_provider(handler)

    result = provider.poll_code(
        upstream_token='orchids@example.com',
        timeout_seconds=1,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=after_ts,
    )

    assert result.status == 'success'
    assert result.code == '123456'
    assert result.message_id == 'msg_new'
    assert result.received_at == '2026-04-03T11:10:00Z'


def test_chatgpt_uk_poll_code_after_ts_prefers_detail_created_at_over_message(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, 'sleep', lambda _: None)
    after_ts = int(datetime(2026, 4, 3, 11, 0, 0, tzinfo=timezone.utc).timestamp() * 1000)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == '/api/emails':
            return httpx.Response(
                200,
                json={
                    'data': [
                        {
                            'id': 'msg_detail_new',
                            'createdAt': '2026-04-03T10:30:00Z',
                        },
                    ],
                },
            )
        if request.url.path == '/api/email/msg_detail_new':
            return httpx.Response(
                200,
                json={
                    'data': {
                        'id': 'msg_detail_new',
                        'subject': 'Your Orchids verification code',
                        'text': 'Your verification code is 777777',
                        'html': '<p>Your verification code is <strong>777777</strong></p>',
                        'createdAt': '2026-04-03T11:10:00Z',
                        'from': 'noreply@orchids.app',
                    },
                },
            )
        raise AssertionError(f'unexpected request: {request.method} {request.url}')

    provider = _make_provider(handler)

    result = provider.poll_code(
        upstream_token='orchids@example.com',
        timeout_seconds=1,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=after_ts,
    )

    assert result.status == 'success'
    assert result.code == '777777'
    assert result.message_id == 'msg_detail_new'
    assert result.received_at == '2026-04-03T11:10:00Z'
