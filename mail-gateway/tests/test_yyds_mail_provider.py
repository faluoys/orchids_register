import json
from datetime import datetime, timezone

import httpx

import mail_gateway.providers.yyds_mail as yyds_mail_module
from mail_gateway.providers.yyds_mail import YYDSMailProvider


def _make_provider(handler) -> YYDSMailProvider:
    client = httpx.Client(
        transport=httpx.MockTransport(handler),
        base_url='https://maliapi.215.im',
    )
    return YYDSMailProvider(
        base_url='https://maliapi.215.im/v1',
        api_key='AC-yyds-test-key',
        client=client,
    )


def test_yyds_provider_acquires_persistent_inbox_via_api() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == 'POST'
        assert request.url.path == '/v1/me/inboxes'
        assert request.headers['X-API-Key'] == 'AC-yyds-test-key'
        assert json.loads(request.content.decode('utf-8')) == {
            'prefix': 'orchids',
            'domain': 'example.com',
        }
        return httpx.Response(
            200,
            json={
                'success': True,
                'data': {
                    'id': 'ibox_123',
                    'address': 'orchids@example.com',
                    'inboxType': 'persistent',
                    'isActive': True,
                    'createdAt': '2026-04-03T10:00:00Z',
                },
            },
        )

    provider = _make_provider(handler)

    acquired = provider.acquire_inbox(
        project_code='orchids',
        domain='example.com',
        metadata={'prefix': 'orchids'},
    )

    assert acquired.address == 'orchids@example.com'
    assert acquired.upstream_token == 'orchids@example.com'
    assert acquired.upstream_ref == 'inbox:ibox_123'
    assert acquired.expires_at is None


def test_yyds_provider_poll_code_reads_message_detail_and_honors_after_ts(monkeypatch) -> None:
    monkeypatch.setattr(yyds_mail_module.time, 'sleep', lambda _: None)
    after_ts = int(datetime(2026, 4, 3, 10, 5, 0, tzinfo=timezone.utc).timestamp() * 1000)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.headers['X-API-Key'] == 'AC-yyds-test-key'
        if request.url.path == '/v1/messages':
            assert request.method == 'GET'
            assert request.url.params['address'] == 'orchids@example.com'
            return httpx.Response(
                200,
                json={
                    'success': True,
                    'data': {
                        'messages': [
                            {
                                'id': 'msg_old',
                                'subject': 'Old code',
                                'createdAt': '2026-04-03T10:00:00Z',
                            },
                            {
                                'id': 'msg_new',
                                'subject': 'Your Orchids verification code',
                                'createdAt': '2026-04-03T10:10:00Z',
                            },
                        ],
                        'total': 2,
                        'unreadCount': 1,
                    },
                },
            )
        if request.url.path == '/v1/messages/msg_new':
            assert request.method == 'GET'
            assert request.url.params['address'] == 'orchids@example.com'
            return httpx.Response(
                200,
                json={
                    'success': True,
                    'data': {
                        'id': 'msg_new',
                        'subject': 'Your Orchids verification code',
                        'text': 'Your verification code is 654321',
                        'html': ['<p>Your verification code is <strong>654321</strong></p>'],
                        'createdAt': '2026-04-03T10:10:00Z',
                        'from': {
                            'name': 'Orchids',
                            'address': 'noreply@orchids.app',
                        },
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


def test_yyds_provider_poll_code_returns_timeout_when_no_message_matches(monkeypatch) -> None:
    clock = {'value': 0.0}

    def fake_time() -> float:
        clock['value'] += 0.6
        return clock['value']

    monkeypatch.setattr(yyds_mail_module.time, 'time', fake_time)
    monkeypatch.setattr(yyds_mail_module.time, 'sleep', lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == '/v1/messages'
        return httpx.Response(
            200,
            json={
                'success': True,
                'data': {
                    'messages': [],
                    'total': 0,
                    'unreadCount': 0,
                },
            },
        )

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


def test_yyds_provider_release_keeps_persistent_inbox() -> None:
    provider = YYDSMailProvider(base_url='https://maliapi.215.im/v1', api_key='AC-yyds-test-key', client=httpx.Client())

    provider.release_inbox('inbox:ibox_123', 'orchids@example.com')