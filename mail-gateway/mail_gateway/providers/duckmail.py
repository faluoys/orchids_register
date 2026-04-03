from mail_gateway.providers.base import InboxProvider


class DuckMailProvider(InboxProvider):
    def acquire_inbox(self, project_code: str | None, domain: str | None, metadata: dict[str, str]):
        raise RuntimeError('DuckMail adapter not implemented')

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ):
        raise RuntimeError('DuckMail adapter not implemented')

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None
