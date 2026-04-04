from mail_gateway.config import Settings
from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult
from mail_gateway.providers.duckmail import DuckMailProvider
from mail_gateway.providers.luckmail import LuckMailProvider
from mail_gateway.providers.mail_chatgpt_uk import MailChatGPTUKProvider
from mail_gateway.providers.yyds_mail import YYDSMailProvider


class StubLuckMailProvider(InboxProvider):
    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        address = 'user2@hotmail.com' if domain and domain.strip().lower().lstrip('@') == 'hotmail.com' else 'user1@outlook.com'
        token = 'tok_hotmail' if address == 'user2@hotmail.com' else 'tok_abc123'
        purchase_id = '2' if address == 'user2@hotmail.com' else '1'
        return AcquiredInbox(
            address=address,
            upstream_token=token,
            upstream_ref=f'purchase:{purchase_id}',
        )

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult:
        return PollResult(
            status='success',
            code='482910',
            message_id='msg_001',
            received_at='2026-04-02T16:10:20Z',
            summary={
                'from': 'info@orchids.app',
                'subject': 'Your verification code',
            },
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


class StubYYDSMailProvider(InboxProvider):
    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        prefix = metadata.get('prefix') or project_code or 'user'
        normalized_domain = (domain or 'example.com').strip().lower().lstrip('@')
        address = f'{prefix}@{normalized_domain}'
        return AcquiredInbox(
            address=address,
            upstream_token=address,
            upstream_ref='inbox:ibox_stub',
        )

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult:
        return PollResult(
            status='success',
            code='482910',
            message_id='msg_stub',
            received_at='2026-04-03T10:10:00Z',
            summary={
                'from': 'noreply@orchids.app',
                'subject': 'Your verification code',
            },
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


class StubMailChatGPTUKProvider(InboxProvider):
    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        prefix = metadata.get('prefix') or project_code or 'user'
        normalized_domain = (domain or 'chatgpt.org.uk').strip().lower().lstrip('@')
        address = f'{prefix}@{normalized_domain}'
        return AcquiredInbox(
            address=address,
            upstream_token=address,
            upstream_ref='inbox:mail_chatgpt_uk_stub',
        )

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult:
        return PollResult(
            status='success',
            code='482910',
            message_id='msg_mail_chatgpt_uk_stub',
            received_at='2026-04-03T10:10:00Z',
            summary={
                'from': 'support@mail.chatgpt.org.uk',
                'subject': 'Your verification code',
            },
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


def build_providers(settings: Settings, testing: bool = False) -> dict[str, InboxProvider]:
    return {
        'luckmail': StubLuckMailProvider()
        if testing
        else LuckMailProvider(settings.luckmail_base_url, settings.luckmail_api_key),
        'yyds_mail': StubYYDSMailProvider()
        if testing
        else YYDSMailProvider(settings.yyds_base_url, settings.yyds_api_key),
        'mail_chatgpt_uk': StubMailChatGPTUKProvider()
        if testing
        else MailChatGPTUKProvider(settings.mail_chatgpt_uk_base_url, settings.mail_chatgpt_uk_api_key),
        'duckmail': DuckMailProvider(),
    }
