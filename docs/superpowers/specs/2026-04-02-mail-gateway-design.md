# Orchids Mail Gateway Design

Date: 2026-04-02
Status: Approved in conversation, pending user review of written spec
Scope: Orchids desktop app inbox integration refactor

## Context

The current Orchids desktop app mixes several historical inbox access approaches:

- `freemail` custom API integrated into the registration workflow
- an unused worker-style `/health` + `/code` flow
- an older `tempmail.lol` implementation that exists in the repo but is not part of the main registration path

This makes the client tightly coupled to one inbox provider shape and unsuitable for adding multiple upstream providers with different protocols.

The target providers are:

- YYDS Mail: `https://vip.215.im/docs`
- DuckMail: `https://www.duckmail.sbs/zh/api-docs`
- LuckMail: `https://mails.luckyous.com/user/api-doc`

Additional constraint:

- Turnstile solving must remain a separate responsibility handled by the existing local `TurnstileSolver` Python service.

## Goals

- Replace provider-specific inbox logic in the desktop client with a single stable inbox gateway integration.
- Keep supplier credentials out of the desktop app.
- Support adding new inbox suppliers without changing the Orchids desktop registration flow again.
- Make LuckMail purchased inboxes usable through the same client-side workflow as temporary inbox providers.
- Minimize first-phase risk by shipping a focused initial implementation.

## Non-Goals

- Rewriting the existing Turnstile solver into the new gateway
- Supporting direct client-to-provider integrations in phase 1
- Implementing all providers at once in the first delivery
- Building real-time push delivery or WebSocket inbox events in phase 1
- Designing a multi-tenant SaaS control plane for the gateway

## Options Considered

### Option 1: Desktop client directly integrates each provider

Pros:

- no extra service to run
- shortest path for one provider

Cons:

- every provider addition requires desktop code changes and a new build
- API keys or provider secrets must live in the client
- provider-specific edge cases leak into Rust/Tauri code
- LuckMail purchased inbox semantics do not match temp inbox semantics cleanly

Conclusion:

- rejected as the long-term architecture

### Option 2: Desktop client only talks to a unified mail gateway

Pros:

- strongest boundary between app and providers
- provider changes stay server-side
- secrets stay off the client
- easiest expansion path

Cons:

- requires a second service
- introduces state management in the gateway

Conclusion:

- accepted as the production direction

### Option 3: Desktop keeps a provider abstraction but production uses gateway by default

Pros:

- keeps the client architecture clean
- allows future direct adapters if ever needed
- avoids hard-coding `freemail` semantics into the new design

Cons:

- slightly more client refactor than a pure hard-coded gateway call

Conclusion:

- accepted as the implementation direction for the desktop codebase

## Recommended Architecture

There will be two external supporting services:

1. `TurnstileSolver`
2. `mail-gateway`

Responsibilities:

- `TurnstileSolver`: local Python service only for Cloudflare Turnstile solving
- `mail-gateway`: provider-facing Python service for inbox allocation, polling, normalization, and cleanup
- Orchids desktop app: registration orchestrator that uses a stable gateway inbox contract

High-level flow:

1. Desktop app requests an inbox session from `mail-gateway`
2. Gateway allocates an upstream inbox resource from the selected provider
3. Desktop app uses the returned address during Orchids sign-up
4. Desktop app requests code polling from `mail-gateway`
5. Gateway retrieves and normalizes messages, extracts the verification code, and returns it
6. Desktop app submits the code to Orchids
7. Desktop app optionally releases the inbox session

## Technology Choice

The mail gateway should be implemented as a separate Python service.

Reasons:

- provider integrations are I/O-bound, not CPU-bound
- iteration speed matters more than raw performance
- supplier response shapes are likely to change over time
- the project already uses Python operationally through `TurnstileSolver`
- LuckMail provides a Python SDK path, and all three providers are natural HTTP integrations

The gateway must still remain a separate codebase or service boundary from `TurnstileSolver` even if both use Python.

## Unified Gateway Contract

The desktop app must stop depending on provider-specific field names and only use the gateway contract below.

### `POST /v1/inboxes/acquire`

Purpose:

- allocate an inbox session for the current registration attempt

Request:

```json
{
  "provider": "luckmail",
  "mode": "purchased",
  "project": "orchids",
  "domain": null,
  "prefix": null,
  "quantity": 1,
  "metadata": {
    "tag_name": "orchids-ready",
    "mark_tag_name": "orchids-used"
  }
}
```

Response:

```json
{
  "session_id": "ses_01JABC...",
  "address": "user1@outlook.com",
  "provider": "luckmail",
  "mode": "purchased",
  "expires_at": null,
  "upstream_ref": "purchase:12345"
}
```

Rules:

- the desktop app only stores `session_id` and `address`
- upstream tokens stay inside the gateway state store

### `POST /v1/inboxes/{session_id}/poll-code`

Purpose:

- poll the upstream provider until a verification code is found, times out, or fails

Request:

```json
{
  "timeout_seconds": 180,
  "interval_seconds": 2,
  "code_pattern": "\\b(\\d{6})\\b",
  "after_ts": 1712044800000
}
```

Response:

```json
{
  "status": "success",
  "code": "482910",
  "message_id": "msg_001",
  "received_at": "2026-04-02T16:10:20Z",
  "summary": {
    "from": "info@example.com",
    "subject": "Your verification code"
  }
}
```

Status values:

- `pending`
- `success`
- `timeout`
- `failed`

### `GET /v1/inboxes/{session_id}/messages`

Purpose:

- inspect normalized message metadata for diagnostics

### `GET /v1/messages/{message_id}`

Purpose:

- inspect one normalized message detail for diagnostics

### `DELETE /v1/inboxes/{session_id}`

Purpose:

- end the local gateway session and optionally release upstream temp resources

### `GET /health`

Purpose:

- replace the current desktop worker-style health test with gateway readiness information

Response example:

```json
{
  "status": "ok",
  "timestamp": 1775116800000,
  "providers": {
    "luckmail": "enabled",
    "yyds_mail": "disabled",
    "duckmail": "disabled"
  }
}
```

## Provider Mapping

### YYDS Mail

Acquisition:

- upstream `POST /v1/accounts`
- auth via `X-API-Key`
- returns temporary `address` and `token`

Polling:

- `GET /v1/messages`
- `GET /v1/messages/{id}`

Gateway notes:

- natural temp inbox model
- low adaptation complexity

### DuckMail

Acquisition:

- upstream `POST /accounts`
- then `POST /token`

Polling:

- `GET /messages`
- `GET /messages/{id}`

Gateway notes:

- similar to YYDS Mail, but requires a token exchange step after account creation
- medium adaptation complexity

### LuckMail

Chosen usage model:

- purchased inbox flow, not order-based temporary code flow

Acquisition:

- upstream `POST /api/v1/openapi/email/purchases/api-get`
- returns `email_address` and `token`
- gateway creates a local `session_id` and stores the upstream token

Polling:

- preferred path: `GET /api/v1/openapi/email/token/{token}/code`
- diagnostic path: `GET /api/v1/openapi/email/token/{token}/mails`
- detailed inspection path: `GET /api/v1/openapi/email/token/{token}/mails/{message_id}`

Gateway notes:

- not a temp inbox model
- session release must not delete the purchased inbox
- tag assignment can be used during allocation and post-allocation marking

## Desktop Client Changes

### Workflow changes

Current state:

- `src/workflow.rs` directly uses `create_freemail_inbox` and `wait_for_freemail_code`

Target state:

- add a new module such as `src/inbox_gateway.rs`
- registration workflow depends on gateway operations only

New workflow steps:

1. acquire inbox session from gateway
2. use returned email address for Orchids registration
3. after verification mail is sent, request `poll-code` from gateway
4. submit the returned code
5. optionally release the inbox session

### CLI and Tauri argument changes

Remove provider-shaped fields from primary flow:

- `use_freemail`
- `freemail_base_url`
- `freemail_admin_token`
- `freemail_domain_index`

Replace with stable gateway-oriented fields:

- `mail_mode`: `gateway | manual`
- `mail_gateway_base_url`
- `mail_gateway_api_key`
- `mail_provider`
- `mail_provider_mode`
- `mail_project_code`
- `mail_domain`

### UI changes

Replace the current inbox configuration semantics with a gateway-focused page:

- Gateway Base URL
- Gateway API Key
- Provider
- Provider Mode
- Project Code
- Domain

Health test:

- call gateway `/health`

System settings:

- remove visible freemail-specific fields from the active UI path

### Config migration

Keep the existing SQLite config table and migrate keys only.

New keys:

- `mail_gateway_base_url`
- `mail_gateway_api_key`
- `mail_provider`
- `mail_provider_mode`
- `mail_project_code`
- `mail_domain`

Compatibility:

- old `freemail_*` keys may be read during a short transition window
- UI should stop exposing old fields

## Legacy Code Strategy

The following legacy modules should not be deleted in phase 1:

- `src/freemail.rs`
- `src/tempmail.rs`
- worker-style code polling helpers

Phase 1 rule:

- remove them from the main registration path
- leave cleanup for a later stabilization pass

This reduces delivery risk while keeping rollback options.

## Mail Gateway Internal Design

### Suggested structure

```text
mail-gateway/
  app.py
  config.py
  providers/
    base.py
    luckmail.py
    yyds_mail.py
    duckmail.py
  services/
    session_service.py
    code_poll_service.py
    message_normalizer.py
  store/
    sqlite_store.py
  schemas/
    inbox.py
```

### State store

Phase 1 should use SQLite.

Store fields:

- `session_id`
- `provider`
- `mode`
- `address`
- `upstream_token`
- `upstream_ref`
- `project_code`
- `status`
- `last_message_id`
- `created_at`
- `expires_at`

Reasoning:

- single-machine operational model
- easy inspection and debugging
- avoids introducing Redis before it is needed

### Code extraction strategy

Priority order:

1. provider-native `verification_code` if available
2. plain text body
3. HTML body
4. shared regex fallback

This lets the gateway reuse the current regex-based experience while preferring supplier-native extraction when available.

### Error model

The gateway should normalize upstream failures to a small stable set:

- `PROVIDER_AUTH_FAILED`
- `PROVIDER_RATE_LIMITED`
- `INBOX_ACQUIRE_FAILED`
- `CODE_NOT_FOUND`
- `SESSION_EXPIRED`
- `UPSTREAM_BAD_RESPONSE`

Desktop rule:

- display normalized error messages only
- raw upstream payloads stay in gateway logs

## Phased Delivery Plan

### Phase 1

- create Python `mail-gateway`
- implement `luckmail` purchased inbox adapter only
- add stable gateway contract
- switch desktop registration flow to gateway
- keep `TurnstileSolver` unchanged

### Phase 2

- implement `yyds_mail`
- test temporary inbox lifecycle through the same gateway contract

### Phase 3

- implement `duckmail`
- validate token exchange and message detail retrieval

### Phase 4

- remove dead legacy inbox code from the desktop app
- tighten config migration and cleanup

## Testing Strategy

### Gateway tests

- provider adapter unit tests using mocked upstream responses
- contract tests for `acquire`, `poll-code`, `release`
- code extraction tests using representative message payloads

### Desktop tests

- registration workflow test with gateway mock
- config migration test
- health-check UI test

### Manual verification

- run Orchids registration with LuckMail purchased inbox mode
- verify code retrieval
- verify retry and timeout behavior
- verify release behavior does not destroy purchased upstream resources

## Risks and Mitigations

### Risk: supplier response shape changes

Mitigation:

- isolate logic inside provider adapters
- normalize all upstream payloads before exposing them

### Risk: purchased inbox pool misuse in LuckMail

Mitigation:

- mark allocation state through tags
- do not destroy purchased inboxes on session release
- log upstream purchase and session references

### Risk: duplicated logic during migration

Mitigation:

- keep legacy code out of the active path
- defer deletion until gateway flow is stable

### Risk: users confuse Turnstile and mailbox configuration

Mitigation:

- keep services separate
- keep configuration pages clearly separated by purpose

## Final Recommendation

Ship the smallest viable stable architecture:

- keep `TurnstileSolver` separate and unchanged
- introduce a separate Python `mail-gateway`
- first integrate LuckMail purchased inboxes through the gateway
- refactor the desktop app to treat inbox access as a gateway concern, not a provider concern

This gives the strongest long-term compatibility while minimizing initial delivery risk.
