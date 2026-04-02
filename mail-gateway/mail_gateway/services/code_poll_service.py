from mail_gateway.providers.base import InboxProvider
from mail_gateway.store.sqlite_store import SQLiteStore


class CodePollService:
    def __init__(self, store: SQLiteStore, providers: dict[str, InboxProvider]) -> None:
        self.store = store
        self.providers = providers

    def poll_code(
        self,
        session_id: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ):
        record = self.store.get_session(session_id)
        if record is None:
            raise KeyError(session_id)
        provider = self.providers[record.provider]
        result = provider.poll_code(
            record.upstream_token,
            timeout_seconds,
            interval_seconds,
            code_pattern,
            after_ts,
        )
        if result.message_id:
            self.store.update_last_message_id(session_id, result.message_id)
        return result
