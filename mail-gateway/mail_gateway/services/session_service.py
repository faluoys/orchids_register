from datetime import datetime, timezone
from uuid import uuid4

from mail_gateway.providers.base import InboxProvider
from mail_gateway.schemas.inbox import InboxSessionRecord
from mail_gateway.store.sqlite_store import SQLiteStore


class SessionService:
    def __init__(self, store: SQLiteStore, providers: dict[str, InboxProvider]) -> None:
        self.store = store
        self.providers = providers

    def acquire(
        self,
        provider_name: str,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> InboxSessionRecord:
        provider = self.providers[provider_name]
        acquired = provider.acquire_inbox(project_code, domain, metadata)
        record = InboxSessionRecord(
            session_id=f"ses_{uuid4().hex}",
            provider=provider_name,
            mode="purchased",
            address=acquired.address,
            upstream_token=acquired.upstream_token,
            upstream_ref=acquired.upstream_ref,
            project_code=project_code,
            status="active",
            last_message_id=None,
            created_at=datetime.now(timezone.utc).isoformat(),
            expires_at=acquired.expires_at,
        )
        self.store.save_session(record)
        return record

    def release(self, session_id: str) -> None:
        record = self.store.get_session(session_id)
        if record is None:
            raise KeyError(session_id)
        self.providers[record.provider].release_inbox(record.upstream_ref, record.upstream_token)
        self.store.delete_session(session_id)
