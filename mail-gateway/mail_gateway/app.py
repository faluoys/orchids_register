import re
from pathlib import Path
from time import time

import httpx
from fastapi import FastAPI, HTTPException, Response
from pydantic import BaseModel, Field

from mail_gateway.config import Settings, load_settings
from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult
from mail_gateway.providers.luckmail import LuckMailProvider
from mail_gateway.services.code_poll_service import CodePollService
from mail_gateway.services.session_service import SessionService
from mail_gateway.store.sqlite_store import SQLiteStore


class AcquireRequest(BaseModel):
    provider: str
    mode: str = "purchased"
    project: str | None = None
    domain: str | None = None
    prefix: str | None = None
    quantity: int = 1
    metadata: dict[str, str] = Field(default_factory=dict)


class PollCodeRequest(BaseModel):
    timeout_seconds: int = 180
    interval_seconds: float = 2.0
    code_pattern: str = r"\b(\d{6})\b"
    after_ts: int | None = None


class StubLuckMailProvider:
    def acquire_inbox(self, project_code: str | None, metadata: dict[str, str]) -> AcquiredInbox:
        return AcquiredInbox(
            address="user1@outlook.com",
            upstream_token="tok_abc123",
            upstream_ref="purchase:1",
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
            status="success",
            code="482910",
            message_id="msg_001",
            received_at="2026-04-02T16:10:20Z",
            summary={
                "from": "info@orchids.app",
                "subject": "Your verification code",
            },
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


def _raise_bad_request(detail: str) -> None:
    raise HTTPException(status_code=400, detail=detail)


def create_app(settings: Settings | None = None, testing: bool = False) -> FastAPI:
    resolved = settings or load_settings()
    app = FastAPI(title="mail-gateway", version="0.1.0")

    store = SQLiteStore(Path(resolved.database_path))
    store.init_schema()
    providers: dict[str, InboxProvider] = {
        "luckmail": StubLuckMailProvider()
        if testing
        else LuckMailProvider(resolved.luckmail_base_url, resolved.luckmail_api_key),
    }
    session_service = SessionService(store, providers)
    code_poll_service = CodePollService(store, providers)
    app.state.settings = resolved
    app.state.store = store

    @app.get("/health")
    async def health() -> dict[str, object]:
        return {
            "status": "ok",
            "timestamp": int(time() * 1000),
            "providers": resolved.provider_statuses(),
        }

    @app.post("/v1/inboxes/acquire")
    async def acquire_inbox(request: AcquireRequest) -> dict[str, object]:
        if request.provider != "luckmail" or request.mode != "purchased":
            _raise_bad_request("phase 1 only supports luckmail purchased mode")
        if request.domain or request.prefix or request.quantity != 1:
            _raise_bad_request("phase 1 does not support domain, prefix, or quantity overrides")
        try:
            record = session_service.acquire(request.provider, request.project, request.metadata)
        except (RuntimeError, httpx.HTTPError) as exc:
            raise HTTPException(status_code=502, detail="provider error") from exc
        return {
            "session_id": record.session_id,
            "address": record.address,
            "provider": record.provider,
            "mode": record.mode,
            "expires_at": record.expires_at,
            "upstream_ref": record.upstream_ref,
        }

    @app.post("/v1/inboxes/{session_id}/poll-code")
    async def poll_code(session_id: str, request: PollCodeRequest) -> dict[str, object]:
        try:
            re.compile(request.code_pattern)
        except re.error as exc:
            raise HTTPException(status_code=400, detail="invalid code_pattern") from exc
        try:
            result = code_poll_service.poll_code(
                session_id,
                request.timeout_seconds,
                request.interval_seconds,
                request.code_pattern,
                request.after_ts,
            )
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="session not found") from exc
        except (RuntimeError, httpx.HTTPError) as exc:
            raise HTTPException(status_code=502, detail="provider error") from exc
        return {
            "status": result.status,
            "code": result.code,
            "message_id": result.message_id,
            "received_at": result.received_at,
            "summary": result.summary,
        }

    @app.delete("/v1/inboxes/{session_id}")
    async def release_inbox(session_id: str) -> Response:
        try:
            session_service.release(session_id)
        except KeyError as exc:
            raise HTTPException(status_code=404, detail="session not found") from exc
        except (RuntimeError, httpx.HTTPError) as exc:
            raise HTTPException(status_code=502, detail="provider error") from exc
        return Response(status_code=204)

    return app


app = create_app()
