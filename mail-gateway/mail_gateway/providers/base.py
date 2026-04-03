from dataclasses import dataclass
from typing import Protocol


@dataclass
class AcquiredInbox:
    address: str
    upstream_token: str
    upstream_ref: str
    expires_at: str | None = None


@dataclass
class PollResult:
    status: str
    code: str | None
    message_id: str | None
    received_at: str | None
    summary: dict[str, str]


class InboxProvider(Protocol):
    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox: ...

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult: ...

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None: ...
