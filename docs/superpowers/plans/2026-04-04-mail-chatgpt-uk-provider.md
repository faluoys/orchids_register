# Mail ChatGPT UK Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `mail_chatgpt_uk` as a new `mail-gateway` persistent provider and expose its configuration through the Tauri desktop inbox config UI.

**Architecture:** Keep the Orchids desktop app talking only to the existing unified `mail-gateway` contract. Implement GPTMail as a new Python provider adapter, wire it into gateway config and health reporting, then extend the desktop configuration page so users can select and configure the provider without editing YAML.

**Tech Stack:** Python (`fastapi`, `httpx`, `pytest`), Rust/Tauri, React + TypeScript

---

## File Map

- Create: `mail-gateway/mail_gateway/providers/mail_chatgpt_uk.py`
- Create: `mail-gateway/tests/test_mail_chatgpt_uk_provider.py`
- Modify: `mail-gateway/mail_gateway/config.py`
- Modify: `mail-gateway/mail_gateway/providers/registry.py`
- Modify: `mail-gateway/mail_gateway/app.py`
- Modify: `mail-gateway/tests/test_provider_registry.py`
- Modify: `mail-gateway/tests/test_health_api.py`
- Modify: `mail-gateway/tests/test_poll_code_api.py`
- Modify: `src-tauri/src/service_manager.rs`
- Modify: `ui/src/pages/InboxConfigPage.tsx`

### Task 1: Add the GPTMail provider adapter with provider-level tests

**Files:**
- Create: `mail-gateway/mail_gateway/providers/mail_chatgpt_uk.py`
- Create: `mail-gateway/tests/test_mail_chatgpt_uk_provider.py`
- Reference: `mail-gateway/mail_gateway/providers/base.py`
- Reference: `mail-gateway/mail_gateway/providers/yyds_mail.py`

- [ ] **Step 1: Write the failing provider tests**

```python
import json

import httpx

import mail_gateway.providers.mail_chatgpt_uk as mail_chatgpt_uk_module
from mail_gateway.providers.mail_chatgpt_uk import MailChatGPTUKProvider


def _make_provider(handler) -> MailChatGPTUKProvider:
    client = httpx.Client(
        transport=httpx.MockTransport(handler),
        base_url="https://mail.chatgpt.org.uk",
    )
    return MailChatGPTUKProvider(
        base_url="https://mail.chatgpt.org.uk",
        api_key="gpt-test",
        client=client,
    )


def test_provider_acquires_random_inbox_via_get() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "GET"
        assert request.url.path == "/api/generate-email"
        assert request.headers["X-API-Key"] == "gpt-test"
        return httpx.Response(200, json={"success": True, "data": {"email": "rand@example.com"}})

    provider = _make_provider(handler)
    acquired = provider.acquire_inbox(project_code=None, domain=None, metadata={})

    assert acquired.address == "rand@example.com"
    assert acquired.upstream_token == "rand@example.com"
    assert acquired.upstream_ref == "inbox:rand@example.com"
    assert acquired.expires_at is None


def test_provider_acquires_prefix_and_domain_via_post() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "POST"
        assert request.url.path == "/api/generate-email"
        assert json.loads(request.content.decode("utf-8")) == {
            "prefix": "orchids",
            "domain": "example.com",
        }
        return httpx.Response(200, json={"success": True, "data": {"email": "orchids@example.com"}})

    provider = _make_provider(handler)
    acquired = provider.acquire_inbox(
        project_code="orchids",
        domain="example.com",
        metadata={"prefix": "orchids"},
    )

    assert acquired.address == "orchids@example.com"
    assert acquired.upstream_token == "orchids@example.com"
    assert acquired.upstream_ref == "inbox:orchids@example.com"


def test_provider_polls_message_list_and_detail_until_code_found(monkeypatch) -> None:
    monkeypatch.setattr(mail_chatgpt_uk_module.time, "sleep", lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path == "/api/emails":
            return httpx.Response(
                200,
                json={"success": True, "data": [{"id": "m1", "subject": "Code", "createdAt": "2026-04-04T04:20:00Z"}]},
            )
        if request.url.path == "/api/email/m1":
            return httpx.Response(
                200,
                json={
                    "success": True,
                    "data": {
                        "id": "m1",
                        "subject": "Your verification code",
                        "text": "Your verification code is 654321",
                        "html": "<p>Your verification code is <strong>654321</strong></p>",
                        "from": "noreply@orchids.app",
                        "createdAt": "2026-04-04T04:20:00Z",
                    },
                },
            )
        raise AssertionError(f"unexpected request: {request.method} {request.url}")

    provider = _make_provider(handler)
    result = provider.poll_code(
        upstream_token="orchids@example.com",
        timeout_seconds=2,
        interval_seconds=0.1,
        code_pattern=r"(\\d{6})",
        after_ts=None,
    )

    assert result.status == "success"
    assert result.code == "654321"
    assert result.message_id == "m1"
    assert result.summary == {"from": "noreply@orchids.app", "subject": "Your verification code"}


def test_provider_returns_timeout_when_no_matching_message_arrives(monkeypatch) -> None:
    clock = {"value": 0.0}

    def fake_time() -> float:
        clock["value"] += 0.6
        return clock["value"]

    monkeypatch.setattr(mail_chatgpt_uk_module.time, "time", fake_time)
    monkeypatch.setattr(mail_chatgpt_uk_module.time, "sleep", lambda _: None)

    def handler(request: httpx.Request) -> httpx.Response:
        assert request.url.path == "/api/emails"
        return httpx.Response(200, json={"success": True, "data": []})

    provider = _make_provider(handler)
    result = provider.poll_code(
        upstream_token="orchids@example.com",
        timeout_seconds=1,
        interval_seconds=0.1,
        code_pattern=r"(\\d{6})",
        after_ts=None,
    )

    assert result.status == "timeout"
    assert result.code is None
    assert result.summary == {}


def test_provider_acquire_raises_runtime_error_for_invalid_success_payload() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={"success": True, "data": {}})

    provider = _make_provider(handler)

    try:
        provider.acquire_inbox(project_code=None, domain=None, metadata={})
    except RuntimeError as exc:
        assert "data.email" in str(exc)
    else:
        raise AssertionError("expected RuntimeError")
```

- [ ] **Step 2: Run provider tests to verify they fail**

Run: `pytest mail-gateway/tests/test_mail_chatgpt_uk_provider.py -v`
Expected: FAIL with `ModuleNotFoundError` or `ImportError` because `mail_gateway.providers.mail_chatgpt_uk` does not exist yet.

- [ ] **Step 3: Write the minimal provider implementation**

```python
import re
import time

import httpx

from mail_gateway.providers.base import AcquiredInbox, InboxProvider, PollResult


class MailChatGPTUKProvider(InboxProvider):
    def __init__(self, base_url: str, api_key: str, client: httpx.Client | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.client = client or httpx.Client(base_url=self.base_url, timeout=15.0)

    def acquire_inbox(
        self,
        project_code: str | None,
        domain: str | None,
        metadata: dict[str, str],
    ) -> AcquiredInbox:
        prefix = (metadata.get("prefix") or project_code or "").strip() or None
        normalized_domain = (domain or "").strip().lower().lstrip("@") or None

        if prefix or normalized_domain:
            response = self._request(
                "POST",
                "/api/generate-email",
                json={k: v for k, v in {"prefix": prefix, "domain": normalized_domain}.items() if v},
            )
        else:
            response = self._request("GET", "/api/generate-email")

        payload = self._json(response, "acquire")
        data = payload.get("data") or {}
        address = str(data.get("email") or "").strip()
        if not address:
            raise RuntimeError("mail_chatgpt_uk acquire failed: upstream success payload missing data.email")

        return AcquiredInbox(
            address=address,
            upstream_token=address,
            upstream_ref=f"inbox:{address}",
            expires_at=None,
        )

    def poll_code(
        self,
        upstream_token: str,
        timeout_seconds: int,
        interval_seconds: float,
        code_pattern: str,
        after_ts: int | None,
    ) -> PollResult:
        address = upstream_token
        matcher = re.compile(code_pattern)
        deadline = time.time() + timeout_seconds

        while time.time() < deadline:
            messages_payload = self._json(
                self._request("GET", "/api/emails", params={"email": address}),
                "list emails",
            )
            messages = messages_payload.get("data") or []

            for message in messages:
                detail = self._json(
                    self._request("GET", f"/api/email/{message['id']}"),
                    "fetch email detail",
                ).get("data") or {}

                received_at = str(detail.get("createdAt") or message.get("createdAt") or "")
                if after_ts and self._ts_ms(received_at) < after_ts:
                    continue

                text = self._detail_text(detail)
                matched = matcher.search(text)
                if not matched:
                    continue

                return PollResult(
                    status="success",
                    code=matched.group(1),
                    message_id=str(detail.get("id") or message.get("id")),
                    received_at=received_at or None,
                    summary={
                        "from": str(detail.get("from") or ""),
                        "subject": str(detail.get("subject") or message.get("subject") or ""),
                    },
                )

            time.sleep(interval_seconds)

        return PollResult(status="timeout", code=None, message_id=None, received_at=None, summary={})

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None
```

- [ ] **Step 4: Run provider tests to verify they pass**

Run: `pytest mail-gateway/tests/test_mail_chatgpt_uk_provider.py -v`
Expected: all tests in `test_mail_chatgpt_uk_provider.py` PASS.

- [ ] **Step 5: Commit the provider adapter slice**

```bash
git add mail-gateway/mail_gateway/providers/mail_chatgpt_uk.py mail-gateway/tests/test_mail_chatgpt_uk_provider.py
git commit -m "feat: add mail chatgpt uk provider"
```

### Task 2: Wire the provider into gateway config, registry, health, and API validation

**Files:**
- Modify: `mail-gateway/mail_gateway/config.py`
- Modify: `mail-gateway/mail_gateway/providers/registry.py`
- Modify: `mail-gateway/mail_gateway/app.py`
- Modify: `mail-gateway/tests/test_provider_registry.py`
- Modify: `mail-gateway/tests/test_health_api.py`
- Modify: `mail-gateway/tests/test_poll_code_api.py`

- [ ] **Step 1: Extend the gateway-level tests first**

```python
# mail-gateway/tests/test_provider_registry.py
assert set(providers) == {"luckmail", "yyds_mail", "duckmail", "mail_chatgpt_uk"}
chatgpt_acquired = providers["mail_chatgpt_uk"].acquire_inbox("orchids", "example.com", {"prefix": "orchids"})
assert chatgpt_acquired.address == "orchids@example.com"
assert chatgpt_acquired.upstream_token == "orchids@example.com"
assert chatgpt_acquired.upstream_ref == "inbox:orchids@example.com"

# mail-gateway/tests/test_health_api.py
assert payload["providers"]["mail_chatgpt_uk"] == "enabled"
assert payload["providers"]["mail_chatgpt_uk"] == "disabled"

# mail-gateway/tests/test_poll_code_api.py
response = client.post(
    "/v1/inboxes/acquire",
    json={
        "provider": "mail_chatgpt_uk",
        "mode": "persistent",
        "project": "orchids",
        "domain": "example.com",
        "prefix": "orchids",
        "metadata": {},
    },
)
assert response.status_code == 200
assert response.json()["provider"] == "mail_chatgpt_uk"
assert response.json()["address"] == "orchids@example.com"

invalid = client.post(
    "/v1/inboxes/acquire",
    json={
        "provider": "mail_chatgpt_uk",
        "mode": "purchased",
        "project": "orchids",
        "metadata": {},
    },
)
assert invalid.status_code == 400
assert "mail_chatgpt_uk persistent" in invalid.json()["detail"]
```

- [ ] **Step 2: Run the affected gateway tests to verify they fail**

Run: `pytest mail-gateway/tests/test_provider_registry.py mail-gateway/tests/test_health_api.py mail-gateway/tests/test_poll_code_api.py -v`
Expected: FAIL because `Settings` and `build_providers()` do not yet include `mail_chatgpt_uk`, and the acquire API still rejects the provider.

- [ ] **Step 3: Implement config, registry, and API wiring**

```python
# mail-gateway/mail_gateway/config.py
@dataclass(frozen=True)
class Settings:
    host: str
    port: int
    database_path: str
    luckmail_base_url: str
    luckmail_api_key: str
    yyds_base_url: str
    yyds_api_key: str
    mail_chatgpt_uk_base_url: str
    mail_chatgpt_uk_api_key: str

    def provider_statuses(self) -> dict[str, str]:
        return {
            "luckmail": "enabled" if self.luckmail_api_key else "disabled",
            "yyds_mail": "enabled" if self.yyds_api_key else "disabled",
            "duckmail": "disabled",
            "mail_chatgpt_uk": "enabled" if self.mail_chatgpt_uk_api_key else "disabled",
        }
```

```python
# mail-gateway/mail_gateway/providers/registry.py
from mail_gateway.providers.mail_chatgpt_uk import MailChatGPTUKProvider


class StubMailChatGPTUKProvider(InboxProvider):
    def acquire_inbox(self, project_code: str | None, domain: str | None, metadata: dict[str, str]) -> AcquiredInbox:
        prefix = metadata.get("prefix") or project_code or "user"
        normalized_domain = (domain or "example.com").strip().lower().lstrip("@")
        address = f"{prefix}@{normalized_domain}"
        return AcquiredInbox(address=address, upstream_token=address, upstream_ref=f"inbox:{address}")

    def poll_code(self, upstream_token: str, timeout_seconds: int, interval_seconds: float, code_pattern: str, after_ts: int | None) -> PollResult:
        return PollResult(
            status="success",
            code="482910",
            message_id="msg_mail_chatgpt_uk",
            received_at="2026-04-04T04:20:00Z",
            summary={"from": "noreply@orchids.app", "subject": "Your verification code"},
        )

    def release_inbox(self, upstream_ref: str, upstream_token: str) -> None:
        return None
```

```python
# mail-gateway/mail_gateway/app.py
allowed_provider_modes = {
    "luckmail": "purchased",
    "yyds_mail": "persistent",
    "mail_chatgpt_uk": "persistent",
}

if expected_mode is None or request.mode != expected_mode:
    supported = ", ".join(f"{name} {mode}" for name, mode in allowed_provider_modes.items())
    _raise_bad_request(f"supported provider/mode pairs: {supported}")

if request.provider in {"yyds_mail", "mail_chatgpt_uk"} and request.prefix:
    metadata["prefix"] = request.prefix
```

- [ ] **Step 4: Run the gateway tests again**

Run: `pytest mail-gateway/tests/test_provider_registry.py mail-gateway/tests/test_health_api.py mail-gateway/tests/test_poll_code_api.py mail-gateway/tests/test_mail_chatgpt_uk_provider.py -v`
Expected: PASS for registry, health, API, and provider tests.

- [ ] **Step 5: Commit the gateway integration slice**

```bash
git add mail-gateway/mail_gateway/config.py mail-gateway/mail_gateway/providers/registry.py mail-gateway/mail_gateway/app.py mail-gateway/tests/test_provider_registry.py mail-gateway/tests/test_health_api.py mail-gateway/tests/test_poll_code_api.py
git commit -m "feat: wire mail chatgpt uk into mail gateway"
```

### Task 3: Expose provider configuration in the desktop-managed Mail Gateway UI

**Files:**
- Modify: `src-tauri/src/service_manager.rs`
- Modify: `ui/src/pages/InboxConfigPage.tsx`
- Reference: `ui/src/lib/tauri-api.ts`
- Reference: `src-tauri/src/commands/config.rs`

- [ ] **Step 1: Add a UI-focused regression target first**

Use the existing page structure as the acceptance boundary. Before editing code, identify the exact additions that must appear after the change:

```tsx
// InboxConfigPage expectations after this task
DEFAULTS.mail_provider === "mail_chatgpt_uk" || existing default retained with visible guidance
DEFAULTS.mail_provider_mode === "persistent" || existing default retained with visible guidance
configs["mail_chatgpt_uk_base_url"]
configs["mail_chatgpt_uk_api_key"]
guidance text includes "mail_chatgpt_uk + persistent"
provider warning logic checks missing mail_chatgpt_uk_api_key when selected
```

This step is intentionally a UI acceptance checklist instead of a test file because the current Tauri React app does not yet have a page-level automated test harness in this worktree.

- [ ] **Step 2: Update the Tauri-managed service environment mapping**

```rust
// src-tauri/src/service_manager.rs
let mail_chatgpt_uk_base_url = optional_config(
    config,
    "mail_chatgpt_uk_base_url",
    "https://mail.chatgpt.org.uk",
);
let mail_chatgpt_uk_api_key = optional_config(config, "mail_chatgpt_uk_api_key", "");

env.extend([
    ("MAIL_CHATGPT_UK_BASE_URL".to_string(), mail_chatgpt_uk_base_url),
    ("MAIL_CHATGPT_UK_API_KEY".to_string(), mail_chatgpt_uk_api_key),
]);
```

Also extend any config seeding/default map in the same file so desktop-managed startup persists:

```rust
("mail_chatgpt_uk_base_url", "https://mail.chatgpt.org.uk"),
("mail_chatgpt_uk_api_key", ""),
```

- [ ] **Step 3: Update the inbox config page defaults, save payload, warnings, and form fields**

```tsx
// ui/src/pages/InboxConfigPage.tsx
const DEFAULTS = {
  // keep existing defaults that should remain stable
  mail_chatgpt_uk_base_url: "https://mail.chatgpt.org.uk",
  mail_chatgpt_uk_api_key: "",
} as const;

const SAVE_KEYS = [
  // existing keys...
  "mail_chatgpt_uk_base_url",
  "mail_chatgpt_uk_api_key",
] as const;

const CLEARABLE_KEYS = new Set<string>([
  // existing keys...
  "mail_chatgpt_uk_api_key",
]);

if (provider === "mail_chatgpt_uk" && !(configs["mail_chatgpt_uk_api_key"] || "").trim()) {
  warnings.push("当前选择的是 mail_chatgpt_uk，但还没填写 GPTMail API Key。");
}
```

Add the provider source inputs beside the existing LuckMail / YYDS blocks:

```tsx
<div className="form-group">
  <label>GPTMail Base URL</label>
  <input
    type="text"
    value={configs["mail_chatgpt_uk_base_url"] || ""}
    onChange={(event) => updateConfig("mail_chatgpt_uk_base_url", event.target.value)}
    className="input"
    style={{ width: "100%" }}
  />
</div>
<div className="form-group" style={{ marginBottom: 0 }}>
  <label>GPTMail API Key</label>
  <input
    type="password"
    value={configs["mail_chatgpt_uk_api_key"] || ""}
    onChange={(event) => updateConfig("mail_chatgpt_uk_api_key", event.target.value)}
    className="input"
    style={{ width: "100%" }}
  />
</div>
```

Update the registration guidance copy so it explicitly includes the new pairing:

```tsx
<div className="guidance-copy">
  常用组合现在有三种：
  <code>yyds_mail + persistent</code>
  <code>luckmail + purchased</code>
  <code>mail_chatgpt_uk + persistent</code>
</div>
```

- [ ] **Step 4: Build the UI to verify the page compiles**

Run: `npm run build`
Workdir: `ui`
Expected: Vite build completes successfully with no TypeScript errors caused by the new config fields.

- [ ] **Step 5: Commit the desktop configuration slice**

```bash
git add src-tauri/src/service_manager.rs ui/src/pages/InboxConfigPage.tsx
git commit -m "feat: expose mail chatgpt uk desktop config"
```

### Task 4: Run end-to-end verification across Python and desktop-facing paths

**Files:**
- Verify only: current worktree changes

- [ ] **Step 1: Run the focused Python gateway test suite**

Run: `pytest mail-gateway/tests/test_mail_chatgpt_uk_provider.py mail-gateway/tests/test_provider_registry.py mail-gateway/tests/test_health_api.py mail-gateway/tests/test_poll_code_api.py -v`
Expected: PASS across all targeted mail gateway tests.

- [ ] **Step 2: Run the Rust library tests that cover the desktop app behavior**

Run: `cargo test -p orchids-auto-register-portable --lib -- --nocapture`
Expected: PASS, or if an unrelated pre-existing failure exists, capture the exact failing test names and stop before further claims.

- [ ] **Step 3: Rebuild the UI once more from the final integrated state**

Run: `npm run build`
Workdir: `ui`
Expected: PASS with production bundle emitted.

- [ ] **Step 4: Manually smoke-check the desktop config semantics**

Use this checklist in the running app:

- Open inbox config page and confirm GPTMail base URL / API key inputs are visible
- Enter `mail_provider = mail_chatgpt_uk`
- Enter `mail_provider_mode = persistent`
- Save and restart Mail Gateway from the desktop page
- Run health check and confirm `mail_chatgpt_uk` appears in provider status

- [ ] **Step 5: Commit the verified integrated result**

```bash
git add mail-gateway src-tauri/src/service_manager.rs ui/src/pages/InboxConfigPage.tsx
git commit -m "feat: integrate mail chatgpt uk provider"
```

## Self-Review

### Spec coverage

- Provider adapter, acquire/poll/release mapping: covered by Task 1.
- Config and `/health` status wiring: covered by Task 2.
- `mail_chatgpt_uk + persistent` mode enforcement: covered by Task 2.
- Desktop-managed configuration inputs and guidance: covered by Task 3.
- Verification across provider, gateway, Rust, and UI paths: covered by Task 4.

### Placeholder scan

- No `TODO`, `TBD`, or deferred implementation markers remain.
- Each task names exact files and exact commands.
- Each code-writing step includes concrete snippets instead of “implement similar logic”.

### Type consistency

- Provider name is consistently `mail_chatgpt_uk`.
- Config keys are consistently `mail_chatgpt_uk_base_url` and `mail_chatgpt_uk_api_key`.
- Supported mode is consistently `persistent`.
