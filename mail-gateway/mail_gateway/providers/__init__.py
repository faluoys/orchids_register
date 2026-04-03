from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult
from mail_gateway.providers.duckmail import DuckMailProvider
from mail_gateway.providers.luckmail import LuckMailProvider
from mail_gateway.providers.registry import build_providers
from mail_gateway.providers.yyds_mail import YYDSMailProvider

__all__ = [
    'AcquiredInbox',
    'DuckMailProvider',
    'InboxProvider',
    'LuckMailProvider',
    'PollResult',
    'YYDSMailProvider',
    'build_providers',
]
