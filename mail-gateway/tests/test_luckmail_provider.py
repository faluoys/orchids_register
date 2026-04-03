from datetime import datetime, timezone
from types import SimpleNamespace

import mail_gateway.providers.luckmail as luckmail_module
from mail_gateway.providers.luckmail import LuckMailProvider


class FakeLuckMailClient:
    def __init__(self, user) -> None:
        self.user = user


def _make_provider(monkeypatch, user) -> LuckMailProvider:
    monkeypatch.setattr(
        luckmail_module,
        'create_luckmail_client',
        lambda base_url, api_key: FakeLuckMailClient(user),
    )
    return LuckMailProvider(base_url='https://mails.luckyous.com', api_key='AC-test-key')


def test_luckmail_provider_acquires_purchased_inbox_via_sdk(monkeypatch) -> None:
    calls: list[dict[str, object]] = []

    class FakeUser:
        def api_get_purchases(self, count: int, tag_name: str | None = None, mark_tag_name: str | None = None):
            calls.append(
                {
                    'count': count,
                    'tag_name': tag_name,
                    'mark_tag_name': mark_tag_name,
                }
            )
            return [
                SimpleNamespace(
                    id=1,
                    email_address='user1@outlook.com',
                    token='tok_abc123',
                )
            ]

    provider = _make_provider(monkeypatch, FakeUser())

    session = provider.acquire_inbox(
        project_code='orchids',
        domain=None,
        metadata={'tag_name': 'orchids-ready', 'mark_tag_name': 'orchids-used'},
    )

    assert calls == [
        {
            'count': 1,
            'tag_name': 'orchids-ready',
            'mark_tag_name': 'orchids-used',
        }
    ]
    assert session.address == 'user1@outlook.com'
    assert session.upstream_token == 'tok_abc123'
    assert session.upstream_ref == 'purchase:1'


def test_luckmail_provider_acquires_domain_filtered_inbox_and_marks_only_selected(monkeypatch) -> None:
    calls: list[tuple[str, object]] = []

    class FakeUser:
        def api_get_purchases(self, count: int, tag_name: str | None = None, mark_tag_name: str | None = None):
            calls.append(
                (
                    'api_get_purchases',
                    {
                        'count': count,
                        'tag_name': tag_name,
                        'mark_tag_name': mark_tag_name,
                    },
                )
            )
            return [
                SimpleNamespace(
                    id=1,
                    email_address='user1@outlook.com',
                    token='tok_outlook',
                ),
                SimpleNamespace(
                    id=2,
                    email_address='user2@hotmail.com',
                    token='tok_hotmail',
                ),
            ]

        def set_purchase_tag(self, purchase_id: int, tag_name: str | None = None):
            calls.append(
                (
                    'set_purchase_tag',
                    {
                        'purchase_id': purchase_id,
                        'tag_name': tag_name,
                    },
                )
            )

    provider = _make_provider(monkeypatch, FakeUser())

    session = provider.acquire_inbox(
        project_code='orchids',
        domain='@Hotmail.com',
        metadata={'tag_name': 'orchids-ready', 'mark_tag_name': 'orchids-used'},
    )

    assert calls == [
        (
            'api_get_purchases',
            {
                'count': 100,
                'tag_name': 'orchids-ready',
                'mark_tag_name': None,
            },
        ),
        (
            'set_purchase_tag',
            {
                'purchase_id': 2,
                'tag_name': 'orchids-used',
            },
        ),
    ]
    assert session.address == 'user2@hotmail.com'
    assert session.upstream_token == 'tok_hotmail'
    assert session.upstream_ref == 'purchase:2'


def test_luckmail_provider_poll_code_uses_sdk_detail_fallback(monkeypatch) -> None:
    monkeypatch.setattr(luckmail_module.time, 'sleep', lambda _: None)

    class FakeUser:
        def __init__(self) -> None:
            self.detail_calls: list[tuple[str, str]] = []

        def get_token_code(self, token: str):
            assert token == 'tok_fallback'
            return SimpleNamespace(
                has_new_mail=True,
                verification_code=None,
                mail={
                    'message_id': 'msg-001',
                    'received_at': '2026-04-02T10:00:00Z',
                    'from': 'noreply@orchids.com',
                    'subject': 'Orchids verification',
                },
            )

        def get_token_mail_detail(self, token: str, message_id: str):
            self.detail_calls.append((token, message_id))
            return SimpleNamespace(
                message_id='msg-001',
                from_addr='noreply@orchids.com',
                subject='Your Orchids verification',
                body_text='Your verification code is 654321',
                body_html='',
                received_at='2026-04-02T10:00:00Z',
                verification_code='',
            )

    user = FakeUser()
    provider = _make_provider(monkeypatch, user)

    result = provider.poll_code(
        upstream_token='tok_fallback',
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=None,
    )

    assert user.detail_calls == [('tok_fallback', 'msg-001')]
    assert result.status == 'success'
    assert result.code == '654321'
    assert result.message_id == 'msg-001'
    assert result.received_at == '2026-04-02T10:00:00Z'
    assert result.summary['from'] == 'noreply@orchids.com'


def test_luckmail_provider_poll_code_ignores_old_mail_before_after_ts(monkeypatch) -> None:
    monkeypatch.setattr(luckmail_module.time, 'sleep', lambda _: None)
    after_ts = int(datetime(2026, 4, 2, 10, 5, 0, tzinfo=timezone.utc).timestamp() * 1000)

    class FakeUser:
        def __init__(self) -> None:
            self.calls = 0

        def get_token_code(self, token: str):
            self.calls += 1
            if self.calls == 1:
                return SimpleNamespace(
                    has_new_mail=True,
                    verification_code=None,
                    mail={
                        'message_id': 'msg-old',
                        'received_at': '2026-04-02T10:00:00Z',
                        'from': 'noreply@orchids.com',
                        'subject': 'Old code',
                    },
                )
            return SimpleNamespace(
                has_new_mail=True,
                verification_code='222222',
                mail={
                    'message_id': 'msg-new',
                    'received_at': '2026-04-02T10:10:00Z',
                    'from': 'noreply@orchids.com',
                    'subject': 'New code',
                },
            )

        def get_token_mail_detail(self, token: str, message_id: str):
            return SimpleNamespace(
                message_id=message_id,
                from_addr='noreply@orchids.com',
                subject='Detail',
                body_text='111111',
                body_html='',
                received_at='2026-04-02T10:00:00Z',
                verification_code='',
            )

    user = FakeUser()
    provider = _make_provider(monkeypatch, user)

    result = provider.poll_code(
        upstream_token='tok_after_ts',
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r'(\d{6})',
        after_ts=after_ts,
    )

    assert user.calls >= 2
    assert result.status == 'success'
    assert result.code == '222222'
    assert result.message_id == 'msg-new'


def test_luckmail_provider_acquire_inbox_raises_runtime_error_for_invalid_sdk_values(monkeypatch) -> None:
    class FakeUser:
        def api_get_purchases(self, count: int, tag_name: str | None = None, mark_tag_name: str | None = None):
            return [SimpleNamespace(id=None, email_address='user1@outlook.com', token='tok_abc123')]

    provider = _make_provider(monkeypatch, FakeUser())

    try:
        provider.acquire_inbox(
            project_code='orchids',
            domain=None,
            metadata={'tag_name': 'orchids-ready', 'mark_tag_name': 'orchids-used'},
        )
    except RuntimeError as exc:
        assert 'LuckMail acquire failed' in str(exc)
    else:
        raise AssertionError('expected RuntimeError for invalid SDK success values')


def test_luckmail_provider_acquire_inbox_raises_when_domain_has_no_match(monkeypatch) -> None:
    class FakeUser:
        def api_get_purchases(self, count: int, tag_name: str | None = None, mark_tag_name: str | None = None):
            return [
                SimpleNamespace(id=1, email_address='user1@outlook.com', token='tok_abc123'),
                SimpleNamespace(id=2, email_address='user2@gmail.com', token='tok_def456'),
            ]

    provider = _make_provider(monkeypatch, FakeUser())

    try:
        provider.acquire_inbox(
            project_code='orchids',
            domain='hotmail.com',
            metadata={'tag_name': 'orchids-ready', 'mark_tag_name': 'orchids-used'},
        )
    except RuntimeError as exc:
        assert 'hotmail.com' in str(exc)
    else:
        raise AssertionError('expected RuntimeError when no purchased inbox matches requested domain')
