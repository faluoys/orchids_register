# LuckMail Gateway Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current freemail-specific inbox path with a unified mail-gateway integration and ship LuckMail purchased-inbox support end-to-end for Orchids registration.

**Architecture:** Build a separate Python `mail-gateway` service that wraps LuckMail purchased inbox tokens behind a stable `acquire / poll-code / release / health` contract. Add a Rust gateway client to the Orchids core workflow, then migrate Tauri and React config surfaces from freemail-specific keys to gateway-based settings while keeping legacy inbox code out of the active path.

**Tech Stack:** Python 3.11, FastAPI, httpx, sqlite3, pytest; Rust `reqwest` + `serde`; Tauri 2; React 19 + Vite + TypeScript.

---

Current workspace note: this extracted snapshot does not contain a `.git` directory, so commit commands below assume the work is executed from a real git clone. If implementation stays in this snapshot, skip commit commands until the repository is restored.

## File Map

### Create

- `mail-gateway/requirements.txt`
- `mail-gateway/.env.example`
- `mail-gateway/mail_gateway/__init__.py`
- `mail-gateway/mail_gateway/app.py`
- `mail-gateway/mail_gateway/config.py`
- `mail-gateway/mail_gateway/schemas/__init__.py`
- `mail-gateway/mail_gateway/schemas/inbox.py`
- `mail-gateway/mail_gateway/store/__init__.py`
- `mail-gateway/mail_gateway/store/sqlite_store.py`
- `mail-gateway/mail_gateway/providers/__init__.py`
- `mail-gateway/mail_gateway/providers/base.py`
- `mail-gateway/mail_gateway/providers/luckmail.py`
- `mail-gateway/mail_gateway/services/__init__.py`
- `mail-gateway/mail_gateway/services/session_service.py`
- `mail-gateway/mail_gateway/services/code_poll_service.py`
- `mail-gateway/tests/test_health_api.py`
- `mail-gateway/tests/test_sqlite_store.py`
- `mail-gateway/tests/test_luckmail_provider.py`
- `mail-gateway/tests/test_poll_code_api.py`
- `src/inbox_gateway.rs`
- `tests/inbox_gateway.rs`

### Modify

- `src/lib.rs`
- `src/cli.rs`
- `src/workflow.rs`
- `src-tauri/src/commands/register.rs`
- `src-tauri/src/commands/config.rs`
- `src-tauri/src/lib.rs`
- `ui/src/lib/types.ts`
- `ui/src/lib/tauri-api.ts`
- `ui/src/pages/RegisterPage.tsx`
- `ui/src/pages/InboxConfigPage.tsx`
- `ui/src/pages/SettingsPage.tsx`

### Keep but remove from active flow

- `src/freemail.rs`
- `src/tempmail.rs`
- worker-style inbox polling helpers in `src/workflow.rs`

## Task 1: Scaffold the Python mail-gateway health service

**Files:**
- Create: `mail-gateway/requirements.txt`
- Create: `mail-gateway/.env.example`
- Create: `mail-gateway/mail_gateway/__init__.py`
- Create: `mail-gateway/mail_gateway/config.py`
- Create: `mail-gateway/mail_gateway/app.py`
- Test: `mail-gateway/tests/test_health_api.py`

- [ ] **Step 1: Write the failing health API test**

```python
# mail-gateway/tests/test_health_api.py
from fastapi.testclient import TestClient

from mail_gateway.app import create_app
from mail_gateway.config import Settings


def test_health_endpoint_reports_enabled_luckmail_provider() -> None:
    settings = Settings(
        host="127.0.0.1",
        port=8081,
        database_path=":memory:",
        luckmail_base_url="https://mails.luckyous.com",
        luckmail_api_key="AC-test-key",
    )
    client = TestClient(create_app(settings=settings))

    response = client.get("/health")

    assert response.status_code == 200
    payload = response.json()
    assert payload["status"] == "ok"
    assert payload["providers"]["luckmail"] == "enabled"
    assert payload["providers"]["yyds_mail"] == "disabled"
    assert payload["providers"]["duckmail"] == "disabled"
    assert isinstance(payload["timestamp"], int)
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd mail-gateway
pytest tests/test_health_api.py -q
```

Expected: FAIL with `ModuleNotFoundError: No module named 'mail_gateway'` or import errors for `create_app` / `Settings`.

- [ ] **Step 3: Write the minimal FastAPI scaffold**

```python
# mail-gateway/mail_gateway/config.py
from dataclasses import dataclass
import os


@dataclass(frozen=True)
class Settings:
    host: str
    port: int
    database_path: str
    luckmail_base_url: str
    luckmail_api_key: str

    def provider_statuses(self) -> dict[str, str]:
        return {
            "luckmail": "enabled" if self.luckmail_api_key else "disabled",
            "yyds_mail": "disabled",
            "duckmail": "disabled",
        }


def load_settings() -> Settings:
    return Settings(
        host=os.getenv("MAIL_GATEWAY_HOST", "127.0.0.1"),
        port=int(os.getenv("MAIL_GATEWAY_PORT", "8081")),
        database_path=os.getenv("MAIL_GATEWAY_DB", "./data/mail_gateway.db"),
        luckmail_base_url=os.getenv("LUCKMAIL_BASE_URL", "https://mails.luckyous.com"),
        luckmail_api_key=os.getenv("LUCKMAIL_API_KEY", ""),
    )
```

```python
# mail-gateway/mail_gateway/app.py
from time import time

from fastapi import FastAPI

from mail_gateway.config import Settings, load_settings


def create_app(settings: Settings | None = None) -> FastAPI:
    resolved = settings or load_settings()
    app = FastAPI(title="mail-gateway", version="0.1.0")
    app.state.settings = resolved

    @app.get("/health")
    async def health() -> dict[str, object]:
        return {
            "status": "ok",
            "timestamp": int(time() * 1000),
            "providers": resolved.provider_statuses(),
        }

    return app


app = create_app()
```

```text
# mail-gateway/requirements.txt
fastapi==0.116.1
uvicorn==0.35.0
httpx==0.28.1
pytest==8.4.1
```

```env
# mail-gateway/.env.example
MAIL_GATEWAY_HOST=127.0.0.1
MAIL_GATEWAY_PORT=8081
MAIL_GATEWAY_DB=./data/mail_gateway.db
LUCKMAIL_BASE_URL=https://mails.luckyous.com
LUCKMAIL_API_KEY=AC-your-key
```

```python
# mail-gateway/mail_gateway/__init__.py
__all__ = ["app", "config"]
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cd mail-gateway
pytest tests/test_health_api.py -q
```

Expected: PASS with `1 passed`.

- [ ] **Step 5: Commit**

```bash
git add mail-gateway/requirements.txt mail-gateway/.env.example mail-gateway/mail_gateway/__init__.py mail-gateway/mail_gateway/config.py mail-gateway/mail_gateway/app.py mail-gateway/tests/test_health_api.py
git commit -m "feat: scaffold mail gateway health api"
```

## Task 2: Add SQLite-backed session storage

**Files:**
- Create: `mail-gateway/mail_gateway/schemas/__init__.py`
- Create: `mail-gateway/mail_gateway/schemas/inbox.py`
- Create: `mail-gateway/mail_gateway/store/__init__.py`
- Create: `mail-gateway/mail_gateway/store/sqlite_store.py`
- Test: `mail-gateway/tests/test_sqlite_store.py`

- [ ] **Step 1: Write the failing store round-trip test**

```python
# mail-gateway/tests/test_sqlite_store.py
from pathlib import Path

from mail_gateway.schemas.inbox import InboxSessionRecord
from mail_gateway.store.sqlite_store import SQLiteStore


def test_sqlite_store_round_trip(tmp_path: Path) -> None:
    db_path = tmp_path / "mail_gateway.db"
    store = SQLiteStore(db_path)
    store.init_schema()

    record = InboxSessionRecord(
        session_id="ses_test_001",
        provider="luckmail",
        mode="purchased",
        address="user1@outlook.com",
        upstream_token="tok_abc123",
        upstream_ref="purchase:42",
        project_code="orchids",
        status="active",
        last_message_id=None,
        created_at="2026-04-02T10:00:00Z",
        expires_at=None,
    )

    store.save_session(record)
    loaded = store.get_session("ses_test_001")

    assert loaded is not None
    assert loaded.address == "user1@outlook.com"
    assert loaded.upstream_token == "tok_abc123"

    store.update_last_message_id("ses_test_001", "msg_001")
    updated = store.get_session("ses_test_001")
    assert updated is not None
    assert updated.last_message_id == "msg_001"

    store.delete_session("ses_test_001")
    assert store.get_session("ses_test_001") is None
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd mail-gateway
pytest tests/test_sqlite_store.py -q
```

Expected: FAIL with import errors for `InboxSessionRecord` or `SQLiteStore`.

- [ ] **Step 3: Write the minimal schema and store implementation**

```python
# mail-gateway/mail_gateway/schemas/inbox.py
from pydantic import BaseModel


class InboxSessionRecord(BaseModel):
    session_id: str
    provider: str
    mode: str
    address: str
    upstream_token: str
    upstream_ref: str
    project_code: str | None = None
    status: str
    last_message_id: str | None = None
    created_at: str
    expires_at: str | None = None
```

```python
# mail-gateway/mail_gateway/store/sqlite_store.py
import sqlite3
from pathlib import Path

from mail_gateway.schemas.inbox import InboxSessionRecord


class SQLiteStore:
    def __init__(self, db_path: str | Path) -> None:
        self.db_path = str(db_path)

    def _connect(self) -> sqlite3.Connection:
        return sqlite3.connect(self.db_path)

    def init_schema(self) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS inbox_sessions (
                    session_id TEXT PRIMARY KEY,
                    provider TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    address TEXT NOT NULL,
                    upstream_token TEXT NOT NULL,
                    upstream_ref TEXT NOT NULL,
                    project_code TEXT,
                    status TEXT NOT NULL,
                    last_message_id TEXT,
                    created_at TEXT NOT NULL,
                    expires_at TEXT
                )
                """
            )

    def save_session(self, record: InboxSessionRecord) -> None:
        with self._connect() as conn:
            conn.execute(
                """
                INSERT OR REPLACE INTO inbox_sessions (
                    session_id, provider, mode, address, upstream_token,
                    upstream_ref, project_code, status, last_message_id,
                    created_at, expires_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    record.session_id,
                    record.provider,
                    record.mode,
                    record.address,
                    record.upstream_token,
                    record.upstream_ref,
                    record.project_code,
                    record.status,
                    record.last_message_id,
                    record.created_at,
                    record.expires_at,
                ),
            )

    def get_session(self, session_id: str) -> InboxSessionRecord | None:
        with self._connect() as conn:
            row = conn.execute(
                "SELECT session_id, provider, mode, address, upstream_token, upstream_ref, project_code, status, last_message_id, created_at, expires_at FROM inbox_sessions WHERE session_id = ?",
                (session_id,),
            ).fetchone()
        if row is None:
            return None
        return InboxSessionRecord(
            session_id=row[0],
            provider=row[1],
            mode=row[2],
            address=row[3],
            upstream_token=row[4],
            upstream_ref=row[5],
            project_code=row[6],
            status=row[7],
            last_message_id=row[8],
            created_at=row[9],
            expires_at=row[10],
        )

    def update_last_message_id(self, session_id: str, message_id: str) -> None:
        with self._connect() as conn:
            conn.execute(
                "UPDATE inbox_sessions SET last_message_id = ? WHERE session_id = ?",
                (message_id, session_id),
            )

    def delete_session(self, session_id: str) -> None:
        with self._connect() as conn:
            conn.execute("DELETE FROM inbox_sessions WHERE session_id = ?", (session_id,))
```
```python
# mail-gateway/mail_gateway/schemas/__init__.py
from mail_gateway.schemas.inbox import InboxSessionRecord

__all__ = ["InboxSessionRecord"]
```

```python
# mail-gateway/mail_gateway/store/__init__.py
from mail_gateway.store.sqlite_store import SQLiteStore

__all__ = ["SQLiteStore"]
```

Add `pydantic` to requirements:

```text
pydantic==2.11.7
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cd mail-gateway
pytest tests/test_sqlite_store.py -q
```

Expected: PASS with `1 passed`.

- [ ] **Step 5: Commit**

```bash
git add mail-gateway/mail_gateway/schemas/__init__.py mail-gateway/mail_gateway/schemas/inbox.py mail-gateway/mail_gateway/store/__init__.py mail-gateway/mail_gateway/store/sqlite_store.py mail-gateway/tests/test_sqlite_store.py mail-gateway/requirements.txt
git commit -m "feat: add sqlite session store for mail gateway"
```

## Task 3: Implement LuckMail purchased inbox acquisition

**Files:**
- Create: `mail-gateway/mail_gateway/providers/__init__.py`
- Create: `mail-gateway/mail_gateway/providers/base.py`
- Create: `mail-gateway/mail_gateway/providers/luckmail.py`
- Test: `mail-gateway/tests/test_luckmail_provider.py`

- [ ] **Step 1: Write the failing LuckMail acquisition test**

```python
# mail-gateway/tests/test_luckmail_provider.py
import json

import httpx

from mail_gateway.providers.luckmail import LuckMailProvider


def test_luckmail_provider_acquires_purchased_inbox() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        assert request.url.path == "/api/v1/openapi/email/purchases/api-get"
        assert request.headers["X-API-Key"] == "AC-test-key"
        payload = json.loads(request.content.decode("utf-8"))
        assert payload["count"] == 1
        assert payload["tag_name"] == "orchids-ready"
        assert payload["mark_tag_name"] == "orchids-used"
        return httpx.Response(
            200,
            json={
                "code": 0,
                "message": "success",
                "data": [
                    {
                        "id": 1,
                        "email_address": "user1@outlook.com",
                        "token": "tok_abc123",
                        "project_name": "Orchids",
                        "tag_id": 2,
                        "tag_name": "orchids-used",
                    }
                ],
            },
        )

    provider = LuckMailProvider(
        base_url="https://mails.luckyous.com",
        api_key="AC-test-key",
        client=httpx.Client(transport=httpx.MockTransport(handler), base_url="https://mails.luckyous.com"),
    )

    session = provider.acquire_inbox(
        project_code="orchids",
        metadata={"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
    )

    assert session.address == "user1@outlook.com"
    assert session.upstream_token == "tok_abc123"
    assert session.upstream_ref == "purchase:1"
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd mail-gateway
pytest tests/test_luckmail_provider.py -q
```

Expected: FAIL with import errors for `LuckMailProvider`.

- [ ] **Step 3: Write the provider base class and LuckMail adapter**

```python
# mail-gateway/mail_gateway/providers/base.py
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
    def acquire_inbox(self, project_code: str | None, metadata: dict[str, str]) -> AcquiredInbox: ...
    def poll_code(self, upstream_token: str, timeout_seconds: int, interval_seconds: float, code_pattern: str, after_ts: int | None) -> PollResult: ...
    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None: ...
```

```python
# mail-gateway/mail_gateway/providers/luckmail.py
from __future__ import annotations

import re
import time

import httpx

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


class LuckMailProvider(InboxProvider):
    def __init__(self, base_url: str, api_key: str, client: httpx.Client | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.client = client or httpx.Client(base_url=self.base_url, timeout=30.0)

    def acquire_inbox(self, project_code: str | None, metadata: dict[str, str]) -> AcquiredInbox:
        response = self.client.post(
            "/api/v1/openapi/email/purchases/api-get",
            headers={"X-API-Key": self.api_key},
            json={
                "count": 1,
                "tag_name": metadata.get("tag_name", "orchids-ready"),
                "mark_tag_name": metadata.get("mark_tag_name", "orchids-used"),
            },
        )
        response.raise_for_status()
        payload = response.json()
        if payload.get("code") != 0 or not payload.get("data"):
            raise RuntimeError(f"LuckMail acquire failed: {payload}")

        first = payload["data"][0]
        return AcquiredInbox(
            address=first["email_address"],
            upstream_token=first["token"],
            upstream_ref=f"purchase:{first['id']}",
            expires_at=None,
        )

    def poll_code(self, upstream_token: str, timeout_seconds: int, interval_seconds: float, code_pattern: str, after_ts: int | None) -> PollResult:
        deadline = time.time() + timeout_seconds
        regex = re.compile(code_pattern)
        while time.time() < deadline:
            response = self.client.get(f"/api/v1/openapi/email/token/{upstream_token}/code")
            response.raise_for_status()
            payload = response.json()
            if payload.get("code") != 0:
                raise RuntimeError(f"LuckMail poll failed: {payload}")
            data = payload.get("data") or {}
            if data.get("has_new_mail") and data.get("verification_code"):
                mail = data.get("mail") or {}
                return PollResult(
                    status="success",
                    code=data.get("verification_code") or _extract_code(regex, mail),
                    message_id=mail.get("message_id"),
                    received_at=mail.get("received_at"),
                    summary={
                        "from": mail.get("from", ""),
                        "subject": mail.get("subject", ""),
                    },
                )
            time.sleep(max(interval_seconds, 0.5))
        return PollResult(status="timeout", code=None, message_id=None, received_at=None, summary={})

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None


def _extract_code(regex: re.Pattern[str], mail: dict[str, str]) -> str | None:
    text = "\n".join([
        mail.get("subject", ""),
        mail.get("body_text", ""),
        mail.get("body_html", ""),
    ])
    match = regex.search(text)
    return match.group(1) if match else None
```

```python
# mail-gateway/mail_gateway/providers/__init__.py
from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult
from mail_gateway.providers.luckmail import LuckMailProvider

__all__ = ["AcquiredInbox", "InboxProvider", "LuckMailProvider", "PollResult"]
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cd mail-gateway
pytest tests/test_luckmail_provider.py -q
```

Expected: PASS with `1 passed`.

- [ ] **Step 5: Commit**

```bash
git add mail-gateway/mail_gateway/providers/__init__.py mail-gateway/mail_gateway/providers/base.py mail-gateway/mail_gateway/providers/luckmail.py mail-gateway/tests/test_luckmail_provider.py
git commit -m "feat: add luckmail purchased inbox provider"
```
## Task 4: Expose acquire, poll-code, and release API routes

**Files:**
- Create: `mail-gateway/mail_gateway/services/__init__.py`
- Create: `mail-gateway/mail_gateway/services/session_service.py`
- Create: `mail-gateway/mail_gateway/services/code_poll_service.py`
- Modify: `mail-gateway/mail_gateway/app.py`
- Test: `mail-gateway/tests/test_poll_code_api.py`

- [ ] **Step 1: Write the failing API contract test**

```python
# mail-gateway/tests/test_poll_code_api.py
from fastapi.testclient import TestClient

from mail_gateway.app import create_app
from mail_gateway.config import Settings


def test_acquire_then_poll_then_release_returns_unified_contract() -> None:
    settings = Settings(
        host="127.0.0.1",
        port=8081,
        database_path=":memory:",
        luckmail_base_url="https://mails.luckyous.com",
        luckmail_api_key="AC-test-key",
    )
    client = TestClient(create_app(settings=settings, testing=True))

    acquired = client.post(
        "/v1/inboxes/acquire",
        json={
            "provider": "luckmail",
            "mode": "purchased",
            "project": "orchids",
            "metadata": {"tag_name": "orchids-ready", "mark_tag_name": "orchids-used"},
        },
    )
    assert acquired.status_code == 200
    session_id = acquired.json()["session_id"]
    assert acquired.json()["address"] == "user1@outlook.com"

    polled = client.post(
        f"/v1/inboxes/{session_id}/poll-code",
        json={
            "timeout_seconds": 5,
            "interval_seconds": 0.1,
            "code_pattern": "\\b(\\d{6})\\b",
            "after_ts": None,
        },
    )
    assert polled.status_code == 200
    assert polled.json()["status"] == "success"
    assert polled.json()["code"] == "482910"

    released = client.delete(f"/v1/inboxes/{session_id}")
    assert released.status_code == 204
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cd mail-gateway
pytest tests/test_poll_code_api.py -q
```

Expected: FAIL with `404` responses because `/v1/inboxes/*` routes do not exist.

- [ ] **Step 3: Implement session and polling services plus FastAPI routes**

```python
# mail-gateway/mail_gateway/services/session_service.py
from datetime import datetime, timezone
from uuid import uuid4

from mail_gateway.providers.base import InboxProvider
from mail_gateway.schemas.inbox import InboxSessionRecord
from mail_gateway.store.sqlite_store import SQLiteStore


class SessionService:
    def __init__(self, store: SQLiteStore, providers: dict[str, InboxProvider]) -> None:
        self.store = store
        self.providers = providers

    def acquire(self, provider_name: str, project_code: str | None, metadata: dict[str, str]) -> InboxSessionRecord:
        provider = self.providers[provider_name]
        acquired = provider.acquire_inbox(project_code, metadata)
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
            return
        self.providers[record.provider].release_inbox(record.upstream_ref, record.upstream_token)
        self.store.delete_session(session_id)
```

```python
# mail-gateway/mail_gateway/services/code_poll_service.py
from mail_gateway.providers.base import PollResult
from mail_gateway.store.sqlite_store import SQLiteStore


class CodePollService:
    def __init__(self, store: SQLiteStore, providers: dict[str, object]) -> None:
        self.store = store
        self.providers = providers

    def poll_code(self, session_id: str, timeout_seconds: int, interval_seconds: float, code_pattern: str, after_ts: int | None) -> PollResult:
        record = self.store.get_session(session_id)
        if record is None:
            raise KeyError(session_id)
        provider = self.providers[record.provider]
        result = provider.poll_code(record.upstream_token, timeout_seconds, interval_seconds, code_pattern, after_ts)
        if result.message_id:
            self.store.update_last_message_id(session_id, result.message_id)
        return result
```

```python
# mail-gateway/mail_gateway/app.py
from pathlib import Path
from time import time

from fastapi import FastAPI, HTTPException, Response
from pydantic import BaseModel, Field

from mail_gateway.config import Settings, load_settings
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


class StubLuckMailProvider(LuckMailProvider):
    def __init__(self) -> None:
        pass

    def acquire_inbox(self, project_code: str | None, metadata: dict[str, str]):
        from mail_gateway.providers.base import AcquiredInbox
        return AcquiredInbox(address="user1@outlook.com", upstream_token="tok_abc123", upstream_ref="purchase:1")

    def poll_code(self, upstream_token: str, timeout_seconds: int, interval_seconds: float, code_pattern: str, after_ts: int | None):
        from mail_gateway.providers.base import PollResult
        return PollResult(status="success", code="482910", message_id="msg_001", received_at="2026-04-02T16:10:20Z", summary={"from": "info@orchids.app", "subject": "Your verification code"})


def create_app(settings: Settings | None = None, testing: bool = False) -> FastAPI:
    resolved = settings or load_settings()
    app = FastAPI(title="mail-gateway", version="0.1.0")

    store = SQLiteStore(Path(resolved.database_path))
    store.init_schema()
    providers = {
        "luckmail": StubLuckMailProvider() if testing else LuckMailProvider(resolved.luckmail_base_url, resolved.luckmail_api_key),
    }
    session_service = SessionService(store, providers)
    code_poll_service = CodePollService(store, providers)
    app.state.settings = resolved

    @app.get("/health")
    async def health() -> dict[str, object]:
        return {"status": "ok", "timestamp": int(time() * 1000), "providers": resolved.provider_statuses()}

    @app.post("/v1/inboxes/acquire")
    async def acquire_inbox(request: AcquireRequest) -> dict[str, object]:
        if request.provider != "luckmail" or request.mode != "purchased":
            raise HTTPException(status_code=400, detail="phase 1 only supports luckmail purchased mode")
        record = session_service.acquire(request.provider, request.project, request.metadata)
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
            result = code_poll_service.poll_code(session_id, request.timeout_seconds, request.interval_seconds, request.code_pattern, request.after_ts)
        except KeyError:
            raise HTTPException(status_code=404, detail="session not found")
        return {
            "status": result.status,
            "code": result.code,
            "message_id": result.message_id,
            "received_at": result.received_at,
            "summary": result.summary,
        }

    @app.delete("/v1/inboxes/{session_id}")
    async def release_inbox(session_id: str) -> Response:
        session_service.release(session_id)
        return Response(status_code=204)

    return app


app = create_app()
```

```python
# mail-gateway/mail_gateway/services/__init__.py
from mail_gateway.services.code_poll_service import CodePollService
from mail_gateway.services.session_service import SessionService

__all__ = ["CodePollService", "SessionService"]
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
cd mail-gateway
pytest tests/test_poll_code_api.py -q
```

Expected: PASS with `1 passed`.

- [ ] **Step 5: Commit**

```bash
git add mail-gateway/mail_gateway/app.py mail-gateway/mail_gateway/services/__init__.py mail-gateway/mail_gateway/services/session_service.py mail-gateway/mail_gateway/services/code_poll_service.py mail-gateway/tests/test_poll_code_api.py
git commit -m "feat: expose unified inbox acquire and poll api"
```

## Task 5: Add the Rust gateway client and switch the registration workflow

**Files:**
- Create: `src/inbox_gateway.rs`
- Create: `tests/inbox_gateway.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli.rs`
- Modify: `src/workflow.rs`

- [ ] **Step 1: Write the failing Rust gateway tests**

```rust
// tests/inbox_gateway.rs
use orchids_core::inbox_gateway::{AcquireInboxResponse, GatewaySettings, PollCodeResponse};

#[test]
fn gateway_settings_only_enable_gateway_mode_when_url_and_provider_exist() {
    let disabled = GatewaySettings {
        mode: "manual".to_string(),
        base_url: "".to_string(),
        api_key: None,
        provider: "luckmail".to_string(),
        provider_mode: "purchased".to_string(),
        project_code: Some("orchids".to_string()),
        domain: None,
    };
    assert!(!disabled.enabled());

    let enabled = GatewaySettings {
        mode: "gateway".to_string(),
        base_url: "http://127.0.0.1:8081".to_string(),
        api_key: Some("secret".to_string()),
        provider: "luckmail".to_string(),
        provider_mode: "purchased".to_string(),
        project_code: Some("orchids".to_string()),
        domain: None,
    };
    assert!(enabled.enabled());
}

#[test]
fn parse_gateway_responses_from_json() {
    let acquire: AcquireInboxResponse = serde_json::from_str(r#"{
        "session_id": "ses_001",
        "address": "user1@outlook.com",
        "provider": "luckmail",
        "mode": "purchased",
        "expires_at": null,
        "upstream_ref": "purchase:1"
    }"#).unwrap();
    assert_eq!(acquire.address, "user1@outlook.com");

    let poll: PollCodeResponse = serde_json::from_str(r#"{
        "status": "success",
        "code": "482910",
        "message_id": "msg_001",
        "received_at": "2026-04-02T16:10:20Z",
        "summary": { "from": "info@orchids.app", "subject": "Your verification code" }
    }"#).unwrap();
    assert_eq!(poll.code.as_deref(), Some("482910"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test --test inbox_gateway
```

Expected: FAIL with `could not find inbox_gateway in orchids_core`.


- [ ] **Step 3: Write the minimal gateway client and migrate the core workflow**

```rust
// src/inbox_gateway.rs
use std::collections::HashMap;

use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};

use crate::cli::Args;
use crate::errors::AppError;
use crate::http_client::{json_compact, json_or_raw, req_timeout_secs};

#[derive(Debug, Clone)]
pub struct GatewaySettings {
    pub mode: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub provider: String,
    pub provider_mode: String,
    pub project_code: Option<String>,
    pub domain: Option<String>,
}

impl GatewaySettings {
    pub fn from_args(args: &Args) -> Self {
        Self {
            mode: args.mail_mode.clone(),
            base_url: args.mail_gateway_base_url.clone().unwrap_or_default(),
            api_key: args.mail_gateway_api_key.clone(),
            provider: args.mail_provider.clone(),
            provider_mode: args.mail_provider_mode.clone(),
            project_code: args.mail_project_code.clone(),
            domain: args.mail_domain.clone(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.mode.eq_ignore_ascii_case("gateway")
            && !self.base_url.trim().is_empty()
            && !self.provider.trim().is_empty()
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if !self.mode.eq_ignore_ascii_case("gateway") {
            return Ok(());
        }
        if self.base_url.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-gateway-base-url".to_string(),
            ));
        }
        if self.provider.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-provider".to_string(),
            ));
        }
        if self.provider_mode.trim().is_empty() {
            return Err(AppError::Usage(
                "mail_mode=gateway 时必须提供 --mail-provider-mode".to_string(),
            ));
        }
        Ok(())
    }

    fn acquire_url(&self) -> String {
        format!("{}/v1/inboxes/acquire", self.base_url.trim_end_matches('/'))
    }

    fn poll_url(&self, session_id: &str) -> String {
        format!(
            "{}/v1/inboxes/{}/poll-code",
            self.base_url.trim_end_matches('/'),
            session_id
        )
    }

    fn release_url(&self, session_id: &str) -> String {
        format!("{}/v1/inboxes/{}", self.base_url.trim_end_matches('/'), session_id)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquireInboxRequest {
    pub provider: String,
    pub mode: String,
    pub project: Option<String>,
    pub domain: Option<String>,
    pub prefix: Option<String>,
    pub quantity: i32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcquireInboxResponse {
    pub session_id: String,
    pub address: String,
    pub provider: String,
    pub mode: String,
    pub expires_at: Option<String>,
    pub upstream_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PollCodeRequest {
    pub timeout_seconds: i64,
    pub interval_seconds: f64,
    pub code_pattern: String,
    pub after_ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PollCodeResponse {
    pub status: String,
    pub code: Option<String>,
    pub message_id: Option<String>,
    pub received_at: Option<String>,
    #[serde(default)]
    pub summary: HashMap<String, String>,
}

pub fn acquire_inbox(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
) -> Result<AcquireInboxResponse, AppError> {
    let response = with_gateway_api_key(
        client.post(settings.acquire_url()).timeout(req_timeout_secs(timeout)),
        settings.api_key.as_deref(),
    )
    .json(&AcquireInboxRequest {
        provider: settings.provider.clone(),
        mode: settings.provider_mode.clone(),
        project: settings.project_code.clone(),
        domain: settings.domain.clone(),
        prefix: None,
        quantity: 1,
        metadata: HashMap::new(),
    })
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway acquire 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    let payload = json_or_raw(response);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "mail-gateway acquire 失败: HTTP {} -> {}",
            status,
            json_compact(&payload)
        )));
    }

    serde_json::from_value(payload)
        .map_err(|e| AppError::Runtime(format!("mail-gateway acquire 响应解析失败: {}", e)))
}

pub fn poll_code(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
    session_id: &str,
    request: &PollCodeRequest,
) -> Result<PollCodeResponse, AppError> {
    let response = with_gateway_api_key(
        client
            .post(settings.poll_url(session_id))
            .timeout(req_timeout_secs(timeout)),
        settings.api_key.as_deref(),
    )
    .json(request)
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway poll-code 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    let payload = json_or_raw(response);
    if status >= 400 {
        return Err(AppError::Runtime(format!(
            "mail-gateway poll-code 失败: HTTP {} -> {}",
            status,
            json_compact(&payload)
        )));
    }

    serde_json::from_value(payload)
        .map_err(|e| AppError::Runtime(format!("mail-gateway poll-code 响应解析失败: {}", e)))
}

pub fn release_inbox(
    client: &Client,
    timeout: i64,
    settings: &GatewaySettings,
    session_id: &str,
) -> Result<(), AppError> {
    let response = with_gateway_api_key(
        client
            .delete(settings.release_url(session_id))
            .timeout(req_timeout_secs(timeout)),
        settings.api_key.as_deref(),
    )
    .send()
    .map_err(|e| AppError::Runtime(format!("mail-gateway release 请求失败: {}", e)))?;

    let status = response.status().as_u16();
    if status == 204 || status == 404 {
        return Ok(());
    }

    let payload = json_or_raw(response);
    Err(AppError::Runtime(format!(
        "mail-gateway release 失败: HTTP {} -> {}",
        status,
        json_compact(&payload)
    )))
}

fn with_gateway_api_key(builder: RequestBuilder, api_key: Option<&str>) -> RequestBuilder {
    if let Some(token) = api_key.filter(|value| !value.trim().is_empty()) {
        builder.header("X-API-Key", token.trim())
    } else {
        builder
    }
}
```

```rust
// src/lib.rs
pub mod inbox_gateway;
```

```rust
// src/cli.rs
    #[arg(long = "mail-mode", default_value = "gateway", help = "邮箱模式：gateway 或 manual")]
    pub mail_mode: String,

    #[arg(long = "mail-gateway-base-url", default_value = None, help = "mail-gateway 服务地址")]
    pub mail_gateway_base_url: Option<String>,

    #[arg(long = "mail-gateway-api-key", default_value = None, help = "mail-gateway API Key，可留空")]
    pub mail_gateway_api_key: Option<String>,

    #[arg(long = "mail-provider", default_value = "luckmail", help = "邮箱提供商，phase 1 固定为 luckmail")]
    pub mail_provider: String,

    #[arg(long = "mail-provider-mode", default_value = "purchased", help = "邮箱提供商模式，phase 1 固定为 purchased")]
    pub mail_provider_mode: String,

    #[arg(long = "mail-project-code", default_value = None, help = "mail-gateway project 标识，例如 orchids")]
    pub mail_project_code: Option<String>,

    #[arg(long = "mail-domain", default_value = None, help = "指定邮箱域名；LuckMail purchased 模式可留空")]
    pub mail_domain: Option<String>,
```

```rust
// src/workflow.rs
use crate::inbox_gateway::{
    acquire_inbox, poll_code as poll_gateway_code, release_inbox, AcquireInboxResponse,
    GatewaySettings, PollCodeRequest,
};
```

```rust
// src/workflow.rs
    let (mail_client, _) = create_client(None)?;
    let gateway_settings = GatewaySettings::from_args(&args);
    if args.mail_mode.eq_ignore_ascii_case("gateway") {
        gateway_settings.validate()?;
    }

    let mut email = args.email.clone();
    let mut captcha_token = args.captcha_token.clone();
    let mut gateway_session: Option<AcquireInboxResponse> = None;
```

```rust
// src/workflow.rs
    if gateway_settings.enabled() && email.is_none() {
        on_log(LogEntry {
            step: "[0/4]".to_string(),
            message: "向 mail-gateway 申请邮箱...".to_string(),
            level: "info".to_string(),
            timestamp: timestamp(),
        });

        let acquired = acquire_inbox(&mail_client, args.timeout, &gateway_settings)?;
        email = Some(acquired.address.clone());
        gateway_session = Some(acquired.clone());

        on_log(LogEntry {
            step: "[0/4]".to_string(),
            message: format!("gateway 邮箱: {}", acquired.address),
            level: "info".to_string(),
            timestamp: timestamp(),
        });
    } else if email.is_none() {
        return Err(usage_error("mail_mode=manual 时，必须提供 --email"));
    }
```

```rust
// src/workflow.rs
    let mut email_code = args.email_code.clone();
    if email_code.is_none() && gateway_session.is_some() {
        let session = gateway_session.as_ref().unwrap();
        on_log(LogEntry {
            step: "[4/4]".to_string(),
            message: "通过 mail-gateway 轮询验证码...".to_string(),
            level: "info".to_string(),
            timestamp: timestamp(),
        });

        let polled = poll_gateway_code(
            &mail_client,
            args.timeout,
            &gateway_settings,
            &session.session_id,
            &PollCodeRequest {
                timeout_seconds: args.poll_timeout,
                interval_seconds: args.poll_interval,
                code_pattern: args.code_pattern.clone(),
                after_ts: Some(prepare_start_ms),
            },
        )?;

        match polled.status.as_str() {
            "success" => {
                let code = polled
                    .code
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| AppError::Runtime("mail-gateway 返回 success 但 code 为空".to_string()))?;
                email_code = Some(code.clone());
                result.email_code = Some(code.clone());
                if let Some(obj) = result_payload.as_object_mut() {
                    obj.insert("email_code".to_string(), Value::String(code.clone()));
                }
                on_log(LogEntry {
                    step: "[4/4]".to_string(),
                    message: format!("已提取验证码: {}", code),
                    level: "info".to_string(),
                    timestamp: timestamp(),
                });
            }
            "timeout" => {
                return Err(AppError::Runtime(format!(
                    "在 {} 秒内未通过 mail-gateway 拉取到验证码",
                    args.poll_timeout
                )));
            }
            status => {
                return Err(AppError::Runtime(format!(
                    "mail-gateway poll-code 失败: status={}, summary={:?}",
                    status,
                    polled.summary
                )));
            }
        }
    }

    if email_code.is_none() {
        if let Some(session) = gateway_session.as_ref() {
            let _ = release_inbox(&mail_client, args.timeout, &gateway_settings, &session.session_id);
        }
        on_log(LogEntry {
            step: "[4/4]".to_string(),
            message: "未提供验证码，且当前流程未获取可自动拉码的 gateway 会话".to_string(),
            level: "warn".to_string(),
            timestamp: timestamp(),
        });
        if !args.result_json.is_empty() {
            let output_path = save_result_json(&args.result_json, &result_payload)?;
            on_log(LogEntry {
                step: "[4/4]".to_string(),
                message: format!("注册结果已写入: {}", output_path),
                level: "info".to_string(),
                timestamp: timestamp(),
            });
        }
        return Ok(result);
    }
```

```rust
// src/workflow.rs
    if let Some(session) = gateway_session.as_ref() {
        let _ = release_inbox(&mail_client, args.timeout, &gateway_settings, &session.session_id);
    }
```

- [ ] **Step 4: Run the Rust tests and compile checks to verify they pass**

Run:

```bash
cargo test --test inbox_gateway
cargo check -p orchids-auto-register
```

Expected: PASS with `2 passed`, followed by a successful `Finished` / `Checking` result for the core crate.

- [ ] **Step 5: Commit**

```bash
git add src/inbox_gateway.rs tests/inbox_gateway.rs src/lib.rs src/cli.rs src/workflow.rs
git commit -m "feat: switch registration workflow to mail gateway"
```

## Task 6: Migrate Tauri commands and React configuration to gateway fields

**Files:**
- Modify: `src-tauri/src/commands/register.rs`
- Modify: `src-tauri/src/commands/config.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `ui/src/lib/types.ts`
- Modify: `ui/src/lib/tauri-api.ts`
- Modify: `ui/src/pages/RegisterPage.tsx`
- Modify: `ui/src/pages/InboxConfigPage.tsx`
- Modify: `ui/src/pages/SettingsPage.tsx`

- [ ] **Step 1: Write the failing shared contract change first**

```rust
// src-tauri/src/commands/register.rs
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterArgs {
    pub email: Option<String>,
    pub password: Option<String>,
    pub captcha_token: Option<String>,
    pub use_capmonster: bool,
    pub captcha_api_url: String,
    pub captcha_timeout: i64,
    pub captcha_poll_interval: f64,
    pub captcha_website_url: String,
    pub captcha_website_key: String,
    pub email_code: Option<String>,
    pub locale: String,
    pub timeout: i64,
    pub mail_mode: String,
    pub mail_gateway_base_url: Option<String>,
    pub mail_gateway_api_key: Option<String>,
    pub mail_provider: String,
    pub mail_provider_mode: String,
    pub mail_project_code: Option<String>,
    pub mail_domain: Option<String>,
    pub poll_timeout: i64,
    pub poll_interval: f64,
    pub code_pattern: String,
    pub debug_email: bool,
    pub test_desktop_session: bool,
    pub proxy: Option<String>,
    pub use_proxy_pool: bool,
    pub proxy_pool_api: String,
}

impl RegisterArgs {
    pub fn to_cli_args(&self) -> orchids_core::cli::Args {
        orchids_core::cli::Args {
            email: self.email.clone(),
            password: self.password.clone(),
            captcha_token: self.captcha_token.clone(),
            use_capmonster: self.use_capmonster,
            captcha_api_url: self.captcha_api_url.clone(),
            captcha_timeout: self.captcha_timeout,
            captcha_poll_interval: self.captcha_poll_interval,
            captcha_website_url: self.captcha_website_url.clone(),
            captcha_website_key: self.captcha_website_key.clone(),
            email_code: self.email_code.clone(),
            locale: self.locale.clone(),
            timeout: self.timeout,
            mail_mode: self.mail_mode.clone(),
            mail_gateway_base_url: self.mail_gateway_base_url.clone(),
            mail_gateway_api_key: self.mail_gateway_api_key.clone(),
            mail_provider: self.mail_provider.clone(),
            mail_provider_mode: self.mail_provider_mode.clone(),
            mail_project_code: self.mail_project_code.clone(),
            mail_domain: self.mail_domain.clone(),
            poll_timeout: self.poll_timeout,
            poll_interval: self.poll_interval,
            code_pattern: self.code_pattern.clone(),
            debug_email: self.debug_email,
            result_json: String::new(),
            test_desktop_session: self.test_desktop_session,
            proxy: self.proxy.clone(),
            use_proxy_pool: self.use_proxy_pool,
            proxy_pool_api: self.proxy_pool_api.clone(),
        }
    }
}

fn apply_register_config(args: &mut RegisterArgs, config: &HashMap<String, String>) {
    if args.proxy.is_none() {
        if let Some(proxy) = config.get("proxy") {
            if !proxy.is_empty() {
                args.proxy = Some(proxy.clone());
            }
        }
    }

    if args.mail_gateway_base_url.is_none() {
        if let Some(v) = config.get("mail_gateway_base_url") {
            if !v.is_empty() {
                args.mail_gateway_base_url = Some(v.clone());
            }
        }
    }
    if args.mail_gateway_api_key.is_none() {
        if let Some(v) = config.get("mail_gateway_api_key") {
            if !v.is_empty() {
                args.mail_gateway_api_key = Some(v.clone());
            }
        }
    }
    if args.mail_provider.trim().is_empty() {
        if let Some(v) = config.get("mail_provider") {
            if !v.is_empty() {
                args.mail_provider = v.clone();
            }
        }
    }
    if args.mail_provider_mode.trim().is_empty() {
        if let Some(v) = config.get("mail_provider_mode") {
            if !v.is_empty() {
                args.mail_provider_mode = v.clone();
            }
        }
    }
    if args.mail_project_code.is_none() {
        if let Some(v) = config.get("mail_project_code") {
            if !v.is_empty() {
                args.mail_project_code = Some(v.clone());
            }
        }
    }
    if args.mail_domain.is_none() {
        if let Some(v) = config.get("mail_domain") {
            if !v.is_empty() {
                args.mail_domain = Some(v.clone());
            }
        }
    }

    if let Some(v) = config.get("captcha_api_url") {
        if !v.is_empty() {
            args.captcha_api_url = v.clone();
        }
    }
    if let Some(v) = config.get("proxy_pool_api") {
        if !v.is_empty() {
            args.proxy_pool_api = v.clone();
        }
    }
}
```

```rust
// src-tauri/src/commands/config.rs
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct MailGatewayHealthResult {
    pub status: String,
    pub timestamp: i64,
    pub providers: HashMap<String, String>,
}

#[tauri::command]
pub async fn test_mail_gateway_health(
    base_url: String,
    api_key: Option<String>,
) -> Result<MailGatewayHealthResult, String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return Err("Base URL 不能为空".to_string());
    }

    let url = format!("{}/health", base_url);

    let result = tokio::task::spawn_blocking(move || {
        let (client, _) = create_client(None).map_err(|e| e.to_string())?;
        let mut req = client.get(&url).timeout(std::time::Duration::from_secs(15));
        if let Some(token) = api_key.filter(|value| !value.trim().is_empty()) {
            req = req.header("X-API-Key", token.trim());
        }

        let resp = req.send().map_err(|e| format!("请求失败: {}", e))?;
        let status_code = resp.status().as_u16();
        let text = resp.text().unwrap_or_default();
        if status_code >= 400 {
            return Err(format!("HTTP {}: {}", status_code, text));
        }

        let data: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("解析响应失败: {}", e))?;
        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp = data
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let providers = data
            .get("providers")
            .and_then(|v| v.as_object())
            .map(|map| {
                map.iter()
                    .map(|(key, value)| (key.clone(), value.as_str().unwrap_or("").to_string()))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        if status != "ok" {
            return Err(format!("健康检查失败: status={}", status));
        }
        if timestamp <= 0 {
            return Err("健康检查失败: 响应缺少有效 timestamp".to_string());
        }

        Ok(MailGatewayHealthResult {
            status,
            timestamp,
            providers,
        })
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))??;

    Ok(result)
}
```

```rust
// src-tauri/src/lib.rs
            commands::config::test_mail_gateway_health,
```

Replace the old `commands::config::test_inbox_health,` entry with the new gateway health command above.

```ts
// ui/src/lib/types.ts
export interface RegisterArgs {
  email: string | null;
  password: string | null;
  captcha_token: string | null;
  use_capmonster: boolean;
  captcha_api_url: string;
  captcha_timeout: number;
  captcha_poll_interval: number;
  captcha_website_url: string;
  captcha_website_key: string;
  email_code: string | null;
  locale: string;
  timeout: number;
  mail_mode: string;
  mail_gateway_base_url: string | null;
  mail_gateway_api_key: string | null;
  mail_provider: string;
  mail_provider_mode: string;
  mail_project_code: string | null;
  mail_domain: string | null;
  poll_timeout: number;
  poll_interval: number;
  code_pattern: string;
  debug_email: boolean;
  test_desktop_session: boolean;
  proxy: string | null;
  use_proxy_pool: boolean;
  proxy_pool_api: string;
}

export interface MailGatewayHealthResult {
  status: string;
  timestamp: number;
  providers: Record<string, string>;
}
```

```ts
// ui/src/lib/tauri-api.ts
import { invoke } from "@tauri-apps/api/core";
import type { Account, AccountGroup, Domain, MailGatewayHealthResult, RegisterArgs } from "./types";

export async function testMailGatewayHealth(
  baseUrl: string,
  apiKey: string | null
): Promise<MailGatewayHealthResult> {
  return invoke("test_mail_gateway_health", { baseUrl, apiKey });
}
```

- [ ] **Step 2: Run the existing frontend build to verify it fails against the old pages**

Run:

```bash
cd ui
npm run build
```

Expected: FAIL with TypeScript errors in `RegisterPage.tsx` or `InboxConfigPage.tsx` because they still reference `use_freemail`, `freemail_*`, or `testInboxHealth`.

- [ ] **Step 3: Finish the Tauri injection and update the React pages**

```rust
// src-tauri/src/commands/register.rs
    if let Ok(conn) = db.lock() {
        if let Ok(config) = db::get_all_config(&conn) {
            apply_register_config(&mut args, &config);
        }
    }
```

Insert that exact block in both `start_registration` and `start_batch_registration` immediately after `db::get_all_config(&conn)` succeeds.

```tsx
// ui/src/pages/RegisterPage.tsx
const defaultArgs: RegisterArgs = {
  email: null,
  password: null,
  captcha_token: null,
  use_capmonster: true,
  captcha_api_url: "http://127.0.0.1:5000",
  captcha_timeout: 180,
  captcha_poll_interval: 3.0,
  captcha_website_url: "https://accounts.orchids.app/",
  captcha_website_key: "0x4AAAAAAAWXJGBD7bONzLBd",
  email_code: null,
  locale: "zh-CN",
  timeout: 30,
  mail_mode: "gateway",
  mail_gateway_base_url: null,
  mail_gateway_api_key: null,
  mail_provider: "luckmail",
  mail_provider_mode: "purchased",
  mail_project_code: "orchids",
  mail_domain: null,
  poll_timeout: 180,
  poll_interval: 2.0,
  code_pattern: "\\b(\\d{6})\\b",
  debug_email: true,
  test_desktop_session: true,
  proxy: null,
  use_proxy_pool: false,
  proxy_pool_api: "https://api.douyadaili.com/proxy/?service=GetUnl&authkey=1KB6xBwGlITDeICSw6BI&num=10&lifetime=1&prot=0&format=txt&cstmfmt=%7Bip%7D%7C%7Bport%7D&separator=%5Cr%5Cn&distinct=1&detail=0&portlen=0",
};

const buildArgs = useCallback(async (): Promise<RegisterArgs> => {
  try {
    const config = await getAllConfig();

    return {
      ...defaultArgs,
      password: config["password"] || null,
      captcha_api_url: config["captcha_api_url"] || defaultArgs.captcha_api_url,
      captcha_timeout: Number(config["captcha_timeout"]) || defaultArgs.captcha_timeout,
      captcha_poll_interval: Number(config["captcha_poll_interval"]) || defaultArgs.captcha_poll_interval,
      captcha_website_key: config["captcha_website_key"] || defaultArgs.captcha_website_key,
      captcha_website_url: config["captcha_website_url"] || defaultArgs.captcha_website_url,
      locale: config["locale"] || defaultArgs.locale,
      timeout: Number(config["timeout"]) || defaultArgs.timeout,
      mail_mode: config["mail_mode"] || defaultArgs.mail_mode,
      mail_gateway_base_url: config["mail_gateway_base_url"] || null,
      mail_gateway_api_key: config["mail_gateway_api_key"] || null,
      mail_provider: config["mail_provider"] || defaultArgs.mail_provider,
      mail_provider_mode: config["mail_provider_mode"] || defaultArgs.mail_provider_mode,
      mail_project_code: config["mail_project_code"] || defaultArgs.mail_project_code,
      mail_domain: config["mail_domain"] || null,
      poll_timeout: Number(config["poll_timeout"]) || defaultArgs.poll_timeout,
      poll_interval: Number(config["poll_interval"]) || defaultArgs.poll_interval,
      proxy: config["proxy"] || null,
      use_proxy_pool: config["use_proxy_pool"] === "true",
      proxy_pool_api: config["proxy_pool_api"] || defaultArgs.proxy_pool_api,
    };
  } catch {
    return defaultArgs;
  }
}, []);
```

```tsx
// ui/src/pages/InboxConfigPage.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { Check, Loader2 } from "lucide-react";
import { getAllConfig, saveConfig, testMailGatewayHealth } from "@/lib/tauri-api";

const DEFAULT_MAIL_GATEWAY_BASE_URL = "http://127.0.0.1:8081";
const DEFAULT_MAIL_MODE = "gateway";
const DEFAULT_MAIL_PROVIDER = "luckmail";
const DEFAULT_MAIL_PROVIDER_MODE = "purchased";
const DEFAULT_MAIL_PROJECT_CODE = "orchids";

const loadConfig = useCallback(async () => {
  setLoading(true);
  try {
    const config = await getAllConfig();
    const next = {
      ...config,
      mail_mode: config["mail_mode"] || DEFAULT_MAIL_MODE,
      mail_gateway_base_url: config["mail_gateway_base_url"] || DEFAULT_MAIL_GATEWAY_BASE_URL,
      mail_gateway_api_key: config["mail_gateway_api_key"] || "",
      mail_provider: config["mail_provider"] || DEFAULT_MAIL_PROVIDER,
      mail_provider_mode: config["mail_provider_mode"] || DEFAULT_MAIL_PROVIDER_MODE,
      mail_project_code: config["mail_project_code"] || DEFAULT_MAIL_PROJECT_CODE,
      mail_domain: config["mail_domain"] || "",
    };
    setConfigs(next);
    await saveConfig({
      mail_mode: next.mail_mode,
      mail_gateway_base_url: next.mail_gateway_base_url,
      mail_gateway_api_key: next.mail_gateway_api_key,
      mail_provider: next.mail_provider,
      mail_provider_mode: next.mail_provider_mode,
      mail_project_code: next.mail_project_code,
      mail_domain: next.mail_domain,
    });
  } finally {
    setLoading(false);
  }
}, []);
```

```tsx
// ui/src/pages/InboxConfigPage.tsx
<div className="config-panel">
  <div className="settings-title">Mail Gateway 连接信息</div>
  <div className="form-group">
    <label>Base URL</label>
    <div style={{ display: "flex", alignItems: "center", gap: 8, maxWidth: 760 }}>
      <input
        type="text"
        value={configs["mail_gateway_base_url"] || ""}
        onChange={(e) => updateConfig("mail_gateway_base_url", e.target.value)}
        placeholder={DEFAULT_MAIL_GATEWAY_BASE_URL}
        className="input"
        style={{ flex: 1, minWidth: 0 }}
      />
      <button
        type="button"
        className="btn btn-sm"
        disabled={healthTesting}
        onClick={async () => {
          const baseUrl = (configs["mail_gateway_base_url"] || DEFAULT_MAIL_GATEWAY_BASE_URL).trim();
          const apiKey = (configs["mail_gateway_api_key"] || "").trim() || null;
          setHealthTesting(true);
          setHealthResult(null);
          setHealthError(null);
          try {
            const res = await testMailGatewayHealth(baseUrl, apiKey);
            setHealthResult(res);
          } catch (e: any) {
            setHealthError(String(e));
          } finally {
            setHealthTesting(false);
          }
        }}
        style={{ minWidth: 86, justifyContent: "center" }}
      >
        {healthTesting ? (
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <Loader2 size={12} className="animate-spin" />
            测试中...
          </span>
        ) : (
          "测试"
        )}
      </button>
    </div>
  </div>
  <div className="form-group">
    <label>API Key</label>
    <input
      type="password"
      value={configs["mail_gateway_api_key"] || ""}
      onChange={(e) => updateConfig("mail_gateway_api_key", e.target.value)}
      placeholder="可选：远程 gateway 鉴权使用"
      className="input"
      style={{ width: "100%", maxWidth: 640 }}
    />
  </div>
  <div className="form-group">
    <label>Provider</label>
    <select
      value={configs["mail_provider"] || DEFAULT_MAIL_PROVIDER}
      onChange={(e) => updateConfig("mail_provider", e.target.value)}
      className="input"
      style={{ width: 220 }}
    >
      <option value="luckmail">luckmail</option>
    </select>
  </div>
  <div className="form-group">
    <label>Provider Mode</label>
    <select
      value={configs["mail_provider_mode"] || DEFAULT_MAIL_PROVIDER_MODE}
      onChange={(e) => updateConfig("mail_provider_mode", e.target.value)}
      className="input"
      style={{ width: 220 }}
    >
      <option value="purchased">purchased</option>
    </select>
  </div>
  <div className="form-group">
    <label>Project Code</label>
    <input
      type="text"
      value={configs["mail_project_code"] || ""}
      onChange={(e) => updateConfig("mail_project_code", e.target.value)}
      placeholder={DEFAULT_MAIL_PROJECT_CODE}
      className="input"
      style={{ width: "100%", maxWidth: 320 }}
    />
  </div>
  <div className="form-group">
    <label>Domain</label>
    <input
      type="text"
      value={configs["mail_domain"] || ""}
      onChange={(e) => updateConfig("mail_domain", e.target.value)}
      placeholder="可选，LuckMail purchased 模式通常留空"
      className="input"
      style={{ width: "100%", maxWidth: 320 }}
    />
  </div>
  <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8, flexWrap: "wrap" }}>
    {healthResult && (
      <span style={{ fontSize: 12, color: "var(--ok)" }}>
        健康检查通过：status={healthResult.status}，providers={Object.entries(healthResult.providers)
          .map(([key, value]) => `${key}:${value}`)
          .join(", ")}
      </span>
    )}
    {healthError && (
      <span style={{ fontSize: 12, color: "var(--error)" }}>{healthError}</span>
    )}
  </div>
</div>
```

```tsx
// ui/src/pages/SettingsPage.tsx
export default function SettingsPage() {
  const [configs, setConfigs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved">("idle");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [proxyTesting, setProxyTesting] = useState(false);
  const [proxyResult, setProxyResult] = useState<{ ip: string; country: string; city: string } | null>(null);
  const [proxyError, setProxyError] = useState<string | null>(null);
```

```tsx
// ui/src/pages/SettingsPage.tsx
<div className="settings-grid">
  <div className="config-panel">
    <div className="settings-title">验证码求解 (本地打码 API)</div>
```

Delete the entire old `Freemail 邮箱服务` panel so gateway configuration only lives in `InboxConfigPage.tsx`.

- [ ] **Step 4: Run the Tauri and frontend builds to verify the migration passes**

Run:

```bash
cargo check -p orchids-auto-register-portable
cd ui
npm run build
```

Expected: PASS for both commands, with no remaining references to `freemail_*`, `use_freemail`, or `testInboxHealth` in the UI build output.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/register.rs src-tauri/src/commands/config.rs src-tauri/src/lib.rs ui/src/lib/types.ts ui/src/lib/tauri-api.ts ui/src/pages/RegisterPage.tsx ui/src/pages/InboxConfigPage.tsx ui/src/pages/SettingsPage.tsx
git commit -m "feat: migrate desktop config to gateway inbox flow"
```

## Task 7: Run the end-to-end smoke verification

**Files:**
- Modify: none
- Verify: `mail-gateway/data/mail_gateway.db` (generated locally during smoke test)
- Verify: `register_result.json` (generated by the CLI smoke run)

- [ ] **Step 1: Start the two supporting services separately**

Run in terminal A:

```powershell
Set-Location mail-gateway
python -m venv .venv
.venv\Scripts\python -m pip install -r requirements.txt
New-Item -ItemType Directory -Force .\data | Out-Null
$env:MAIL_GATEWAY_DB = Join-Path (Get-Location) 'data\mail_gateway.db'
$env:LUCKMAIL_BASE_URL = 'https://mails.luckyous.com'
$env:LUCKMAIL_API_KEY = 'AC-your-real-key'
.venv\Scripts\python -m uvicorn mail_gateway.app:app --host 127.0.0.1 --port 8081
```

Run in terminal B:

```powershell
Set-Location ..\TurnstileSolver
python api_solver.py
```

Expected: `mail-gateway` listens on `127.0.0.1:8081`, and `TurnstileSolver` remains a separate local service on its existing port.

- [ ] **Step 2: Verify the gateway contract before touching the desktop app**

Run in terminal C:

```powershell
Invoke-RestMethod -Method Get -Uri http://127.0.0.1:8081/health | ConvertTo-Json -Depth 5
```

Expected: JSON containing `status = "ok"`, a positive `timestamp`, and `providers.luckmail = "enabled"`.

- [ ] **Step 3: Run one real CLI registration through the gateway**

Run:

```powershell
Set-Location ..
cargo run --bin orchids-auto-register -- `
  --mail-mode gateway `
  --mail-gateway-base-url http://127.0.0.1:8081 `
  --mail-provider luckmail `
  --mail-provider-mode purchased `
  --mail-project-code orchids `
  --use-capmonster `
  --captcha-api-url http://127.0.0.1:5000 `
  --poll-timeout 180 `
  --poll-interval 2 `
  --result-json register_result.json
```

Expected: log output includes `向 mail-gateway 申请邮箱...`, `通过 mail-gateway 轮询验证码...`, and `提交邮箱验证码...`; `register_result.json` contains non-empty `email`, `email_code`, and `register_complete = true`.

- [ ] **Step 4: Perform one manual desktop smoke check**

Manual checklist:
1. 启动桌面应用开发模式，进入“收件配置”页面。
2. 填入 `mail_gateway_base_url = http://127.0.0.1:8081`、`mail_provider = luckmail`、`mail_provider_mode = purchased`、`mail_project_code = orchids`。
3. 点击“测试”，确认页面显示 `status=ok` 且 provider 状态里 `luckmail:enabled`。
4. 进入“自动注册”页面，执行 1 个账号的单次注册。
5. 确认日志里不再出现 `freemail` 字样，而是新的 gateway 申请和轮询日志。

- [ ] **Step 5: Commit**

```bash
git add mail-gateway src src-tauri ui docs/superpowers/specs/2026-04-02-mail-gateway-design.md docs/superpowers/plans/2026-04-02-luckmail-gateway-phase1.md
git commit -m "feat: add luckmail gateway phase 1"
```

## Self-Review

### 1. Spec coverage

- Gateway service scaffolding, state store, LuckMail adapter, and unified API contract are covered by Tasks 1-4.
- Core Rust workflow migration from `freemail` to gateway is covered by Task 5.
- Tauri config injection, gateway health test wiring, and React page migration are covered by Task 6.
- Separate `TurnstileSolver` operation plus real smoke verification are covered by Task 7.
- Review fix applied while writing this plan: frontend verification now uses the repo's real `npm run build` flow instead of assuming a nonexistent Vitest setup.

### 2. Placeholder scan

- Checked for `TBD`, `TODO`, `implement later`, `similar to Task`, and unfinished placeholder wording.
- Result: none remain in the executable steps.

### 3. Type consistency

- Shared config keys are standardized as `mail_mode`, `mail_gateway_base_url`, `mail_gateway_api_key`, `mail_provider`, `mail_provider_mode`, `mail_project_code`, and `mail_domain` across CLI, Tauri, and React.
- Shared gateway endpoints are standardized as `GET /health`, `POST /v1/inboxes/acquire`, `POST /v1/inboxes/{session_id}/poll-code`, and `DELETE /v1/inboxes/{session_id}`.
- Phase 1 provider semantics are standardized as `provider=luckmail` plus `mode=purchased` in both gateway and desktop code.

