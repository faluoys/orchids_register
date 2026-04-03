# Mail ChatGPT UK Provider Design

> Design record for adding `mail_chatgpt_uk` as a new `mail-gateway` provider. This document defines the target behavior before implementation.

Date: 2026-04-04
Status: Approved in conversation, pending user review of written spec
Scope: `mail-gateway` persistent inbox provider integration for `https://mail.chatgpt.org.uk`

## Context

The current `mail-gateway` already exposes a stable inbox contract to the Orchids desktop app:

- `POST /v1/inboxes/acquire`
- `POST /v1/inboxes/{session_id}/poll-code`
- `DELETE /v1/inboxes/{session_id}`

The desktop side is now configured primarily through the Tauri UI instead of manual runtime YAML edits. That means new inbox suppliers should be integrated at the `mail-gateway` provider layer, then surfaced as configuration options in the desktop UI when needed.

The new upstream supplier is GPTMail:

- docs: `https://mail.chatgpt.org.uk/zh/api`
- auth: `X-API-Key` request header or `api_key` query param
- generate address: `GET` or `POST /api/generate-email`
- list emails by address: `GET /api/emails?email=...`
- fetch message detail: `GET /api/email/{id}`
- delete one message: `DELETE /api/email/{id}`
- clear mailbox: `DELETE /api/emails/clear?email=...`

The upstream API is shaped like a temporary mailbox API, but for Orchids we want to expose it as a logical `persistent` provider because the registration workflow expects “same address reused during one activation flow” semantics, not “purchase once and own forever” semantics.

## Goals

- Add a new provider named `mail_chatgpt_uk` to `mail-gateway`.
- Expose it as `persistent` mode in the gateway API contract.
- Let Orchids desktop configure and select this provider without depending on `runtime.local.yaml`.
- Keep the existing session lifecycle contract unchanged for the desktop client.
- Minimize implementation risk by mapping GPTMail to the current provider protocol instead of redesigning gateway session semantics.

## Non-Goals

- Building a new Orchids-side local HTTP proxy for GPTMail
- Replacing `mail-gateway` with direct provider access from Tauri
- Designing long-lived mailbox ownership guarantees outside one acquisition session
- Implementing provider-specific webhooks or push delivery
- Solving upstream quota, abuse controls, or rate-limit policy beyond basic retry-safe behavior inside the current gateway shape

## Options Considered

### Option 1: Add GPTMail directly into the desktop app

Pros:

- fewer moving parts for one provider
- no Python gateway changes

Cons:

- duplicates provider logic in the client
- leaks provider credentials into the desktop app
- forces future desktop rebuilds for every supplier change
- breaks the current “desktop talks to one inbox contract” direction

Conclusion:

- rejected

### Option 2: Add GPTMail as a new `mail-gateway` provider with `persistent` mode

Pros:

- fits the existing architecture
- keeps provider secrets inside gateway config
- lets the desktop continue using one stable contract
- lowest-risk way to add another supplier

Cons:

- needs provider adapter code, config plumbing, tests, and light UI updates
- upstream temporary-mail semantics need a clear mapping to gateway session semantics

Conclusion:

- accepted

### Option 3: Rework `mail-gateway` to support a more generic “temporary mailbox” mode first

Pros:

- more theoretically correct naming for GPTMail
- could help future suppliers that are neither clearly `persistent` nor `purchased`

Cons:

- expands scope immediately
- forces desktop-side config and validation changes before we know they are needed
- delays delivery without solving a current blocking issue

Conclusion:

- rejected for now

## Recommended Design

### Provider identity and mode

- provider name: `mail_chatgpt_uk`
- provider mode exposed by gateway: `persistent`
- desktop config recommendation:
  - `mail_provider = mail_chatgpt_uk`
  - `mail_provider_mode = persistent`

This keeps the external Orchids behavior aligned with how `yyds_mail` is already used today.

### Provider configuration

`mail-gateway` settings will add:

- `mail_chatgpt_uk_base_url`
- `mail_chatgpt_uk_api_key`

Recommended defaults:

- base URL default: `https://mail.chatgpt.org.uk`
- API key default: empty string

`provider_statuses()` must report `mail_chatgpt_uk` as:

- `enabled` when API key is present
- `disabled` when API key is absent

### Acquire flow

`mail_chatgpt_uk.acquire_inbox(project_code, domain, metadata)` will map to GPTMail as follows:

1. Determine desired prefix:
   - use `metadata["prefix"]` if provided
   - otherwise use `project_code` if present
   - otherwise let upstream generate randomly
2. Determine desired domain:
   - use the incoming `domain` if provided
   - otherwise omit it and let upstream choose
3. Call upstream `POST /api/generate-email` when prefix or domain is explicitly requested.
4. Fall back to `GET /api/generate-email` when neither prefix nor domain is provided.
5. Parse the returned mailbox address from `data.email`.

The gateway provider will return:

- `address`: upstream generated email address
- `upstream_token`: the email address itself
- `upstream_ref`: a stable logical ref such as `inbox:<email>`
- `expires_at`: `None` unless upstream starts returning an explicit expiry we decide to trust later

Rationale:

- the current provider protocol expects `poll_code` to receive a single `upstream_token`
- GPTMail message listing is keyed by email address
- using the email address as token is the simplest stable mapping

### Poll flow

`mail_chatgpt_uk.poll_code(upstream_token, timeout_seconds, interval_seconds, code_pattern, after_ts)` will:

1. Treat `upstream_token` as the mailbox address.
2. Poll `GET /api/emails?email=...` until timeout.
3. Ignore messages older than `after_ts` when that timestamp is provided.
4. Visit candidate messages by `GET /api/email/{id}`.
5. Extract verification code from the best available text source in this order:
   - plain text body
   - HTML-to-text fallback
   - subject line as last resort
6. Apply the caller-provided `code_pattern`.
7. Return the first matching `PollResult(status="success", ...)`.
8. Return a timeout-shaped `PollResult` if no code is found before deadline.

Normalization rules:

- `message_id` should be the upstream message id converted to string
- `received_at` should preserve upstream timestamp when available
- `summary` should include at least:
  - `from`
  - `subject`

### Release flow

`mail_chatgpt_uk.release_inbox(upstream_ref, upstream_token)` will initially be a no-op.

Reasoning:

- Orchids does not currently require destructive cleanup to finish a successful registration
- GPTMail has mailbox clearing and single-message deletion endpoints, but using them immediately adds risk without helping the activation path
- leaving release as no-op matches the current lowest-risk integration strategy

This is an intentional phase boundary, not an omission. If mailbox hygiene becomes important later, we can add a phase-2 cleanup policy.

## Required Gateway Changes

### `mail_gateway/providers`

Add a new provider module:

- `mail_gateway/providers/mail_chatgpt_uk.py`

Responsibilities:

- wrap GPTMail HTTP requests
- normalize upstream responses into `AcquiredInbox` and `PollResult`
- raise actionable `RuntimeError` messages for malformed upstream responses

### `mail_gateway/config.py`

Add new settings fields:

- `mail_chatgpt_uk_base_url`
- `mail_chatgpt_uk_api_key`

Also update `provider_statuses()` to include `mail_chatgpt_uk`.

### `mail_gateway/providers/registry.py`

Register the real provider and a testing stub provider for `testing=True`.

The stub must:

- return a stable acquired inbox
- return a deterministic verification code from `poll_code`
- allow API and health tests to cover the new provider without real network access

### `mail_gateway/app.py`

Update provider-mode validation:

- add `mail_chatgpt_uk: persistent` to `allowed_provider_modes`
- replace the hard-coded phase-1 error text so it no longer only mentions `luckmail` and `yyds_mail`

The gateway API contract itself should stay unchanged.

## Required Desktop UI Changes

The desktop app should remain gateway-driven and should not add direct GPTMail calls.

However, the config UI should be extended so users can actually configure this provider through the desktop app:

- add `mail_chatgpt_uk_base_url` field
- add `mail_chatgpt_uk_api_key` field
- add inline guidance showing `mail_chatgpt_uk + persistent` as a supported pair
- keep the current “desktop config is primary, YAML is compatibility path” messaging

If the provider field stays as free text for now, that is acceptable for phase 1 as long as the hints explicitly mention `mail_chatgpt_uk`.

## Error Handling

The provider adapter must distinguish these classes cleanly:

- upstream auth/config failure:
  - missing API key locally
  - upstream 401/403 style failures
- upstream request failure:
  - network timeout
  - non-2xx HTTP response
- upstream shape failure:
  - response JSON missing `success`
  - `success=false`
  - missing `data.email` on acquire
- no-code-yet polling result:
  - mailbox exists but no matching message has arrived

Rules:

- provider-raised errors should preserve enough detail for gateway logs and API callers
- polling should keep retrying on empty mailbox / unmatched mail until deadline
- clearly malformed upstream success payloads should fail fast instead of silently returning broken sessions

## Testing Strategy

Tests should cover at least four layers.

### Provider unit tests

- acquire with random generation
- acquire with explicit prefix/domain
- poll success from listed + fetched message
- poll timeout with no matching code
- malformed upstream payload handling

### Registry tests

- `build_providers(..., testing=True)` includes `mail_chatgpt_uk`
- testing stub behaves consistently

### API tests

- `/health` includes `mail_chatgpt_uk`
- `/v1/inboxes/acquire` accepts `mail_chatgpt_uk + persistent`
- invalid mode for `mail_chatgpt_uk` is rejected
- poll/release flow works through the testing stub

### Desktop smoke verification

- config fields save correctly in Tauri
- Mail Gateway health view shows `mail_chatgpt_uk`
- registration config guidance mentions the provider/mode pair

## Risks and Mitigations

### Upstream rate limiting

Risk:

- GPTMail may rate-limit rapid polling or repeated mailbox generation

Mitigation:

- reuse one acquired address per Orchids activation flow
- keep polling interval caller-controlled
- avoid unnecessary cleanup requests in phase 1

### Upstream schema drift

Risk:

- temporary mailbox APIs sometimes change field names or message detail shapes

Mitigation:

- centralize parsing inside one provider adapter
- fail fast on malformed responses
- cover expected shapes with provider unit tests

### Semantics mismatch between “temporary” and “persistent”

Risk:

- naming may suggest guarantees stronger than GPTMail actually provides

Mitigation:

- treat `persistent` as a gateway workflow mode, not a mailbox lifetime guarantee
- scope support to one registration/activation session

## Implementation Boundary

This spec is intentionally limited to one deliverable:

1. add `mail_chatgpt_uk` to `mail-gateway`
2. expose its config in desktop UI
3. verify gateway and UI paths

It does not include:

- mailbox cleanup automation
- provider prioritization / weighted routing
- fallback chains across multiple providers
- direct Orchids desktop HTTP proxying for GPTMail

## Acceptance Criteria

- `mail-gateway` can acquire, poll, and release a `mail_chatgpt_uk` inbox through the existing API surface
- `mail_chatgpt_uk` is reported in `/health`
- `mail_chatgpt_uk` is valid only with `persistent` mode
- desktop users can configure the provider in Tauri without editing runtime YAML
- automated tests cover provider registration, health, acquire, and poll behavior
