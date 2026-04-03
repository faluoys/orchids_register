# Orchids Desktop Config And Service Management Design

Date: 2026-04-03
Status: Approved in conversation, pending user review of written spec
Scope: Tauri desktop app only

## Context

The current desktop workflow has two configuration sources:

- `config/runtime.local.yaml` for PowerShell startup scripts and external services
- the Tauri local database for values edited in the desktop UI

This causes drift. After editing `runtime.local.yaml`, the user still has to re-enter matching values in the desktop app before registration works.

The user wants the desktop app to become the single control surface. The desktop path should no longer depend on `runtime.local.yaml`.

## Goals

- Make the Tauri local database the only configuration source for the desktop workflow
- Move all service-related settings into the desktop UI
- Let the desktop app start and stop `mail-gateway` and `TurnstileSolver`
- Keep the existing desktop registration flow working with minimal behavioral change

## Non-Goals

- Refactoring the standalone CLI registration path in this phase
- Deleting historical YAML files or PowerShell scripts immediately
- Replacing Python services with Rust implementations

## Options Considered

### Option 1: App-managed config and app-managed services

The desktop app stores all settings and launches both Python services itself.

Pros:

- one configuration source
- no manual sync step
- best desktop user experience

Cons:

- requires process lifecycle management in Tauri

Conclusion:

- accepted

### Option 2: App-managed config, external scripts still start services

Pros:

- smaller backend change

Cons:

- user still manages processes manually
- desktop flow is not self-contained

Conclusion:

- rejected

### Option 3: UI writes back to `runtime.local.yaml`

Pros:

- easiest short-term compatibility

Cons:

- still effectively dual-write
- future drift and edge cases remain

Conclusion:

- rejected

## Recommended Architecture

The Tauri app becomes the orchestrator for both configuration and service lifecycle.

### Single source of truth

All desktop settings are stored in the Tauri SQLite config table. The desktop registration commands already read from this store; this design extends the same source to external services.

### Service lifecycle

Tauri adds commands to:

- start `mail-gateway`
- stop `mail-gateway`
- start `TurnstileSolver`
- stop `TurnstileSolver`
- query service status

Each start command builds process arguments from the current config values and injects required environment variables.

### Process model

- `mail-gateway` is launched from `mail-gateway/` using `conda run -n <env> python -m uvicorn mail_gateway.app:app ...`
- `TurnstileSolver` is launched from `TurnstileSolver/` using `conda run -n <env> python api_solver.py ...`
- Tauri stores child-process handles in application state
- repeated start requests do not create duplicate processes
- stop requests terminate only the managed child process for that service

## Configuration Model

The UI must expose and persist the following settings.

### Shared runtime

- `conda_env`

### Mail gateway

- `mail_gateway_host`
- `mail_gateway_port`
- `mail_gateway_database_path`
- `luckmail_base_url`
- `luckmail_api_key`
- `yyds_base_url`
- `yyds_api_key`

### Turnstile solver

- `turnstile_host`
- `turnstile_port`
- `turnstile_thread`
- `turnstile_browser_type`
- `turnstile_headless`
- `turnstile_debug`
- `turnstile_proxy`
- `turnstile_random`

### Existing registration settings

Keep the current desktop config keys already used by registration:

- `mail_mode`
- `mail_gateway_base_url`
- `mail_gateway_api_key`
- `mail_provider`
- `mail_provider_mode`
- `mail_project_code`
- `mail_domain`
- `captcha_api_url`
- `captcha_timeout`
- `captcha_poll_interval`
- `proxy`
- `use_proxy_pool`
- `proxy_pool_api`

## Frontend Changes

### Inbox configuration page

Expand the current page from gateway client settings into full mail-gateway service configuration:

- bind service host and port
- bind provider base URLs and API keys
- bind gateway database path
- keep provider selection and health check

### Settings page

Add a dedicated `TurnstileSolver` section and runtime section:

- conda environment
- solver host and port
- solver thread count and browser type
- headless, debug, proxy, random toggles

### Service controls

Expose explicit actions in the UI:

- start service
- stop service
- restart service
- show current status and last error

Recommended placement:

- one compact status card for `mail-gateway`
- one compact status card for `TurnstileSolver`

## Backend Changes

### App state

Extend Tauri state to track managed processes and service status metadata.

Minimum tracked fields per service:

- running flag
- pid when available
- last start time
- last error message

### Commands

Add Tauri commands for:

- `get_service_status`
- `start_mail_gateway`
- `stop_mail_gateway`
- `start_turnstile_solver`
- `stop_turnstile_solver`

Existing config CRUD commands remain the persistence layer.

### Path handling

Relative paths such as `mail-gateway/data/mail_gateway.db` are resolved against the repository root for development builds. The implementation should make this resolution explicit instead of relying on the old PowerShell helpers.

## Registration Flow Impact

Desktop registration continues to read its runtime values from the config table. The behavioral change is only that the required dependent services are now configured and launched from inside the app.

The registration page should surface clear preflight failures when:

- `mail-gateway` is not running
- `TurnstileSolver` is not running
- required API keys are missing for the selected provider

## Error Handling

- starting a service with missing required config returns a structured error
- starting an already-running service returns success with current status
- failed child-process starts preserve stderr-derived error context when possible
- stopping a non-running service is a no-op success

## Migration

No automatic import from `runtime.local.yaml` is required in this phase.

Defaults may still mirror the historical YAML template, but the desktop path must work without that file existing.

PowerShell scripts and YAML examples remain in the repository for manual or legacy workflows, but they are no longer part of the supported desktop path.

## Testing

- Rust unit tests for process command construction and config validation
- Rust tests for service status transitions
- manual desktop verification:
  - save config
  - start both services from UI
  - run health check
  - complete one desktop registration
  - stop both services from UI

## Risks

- `conda` may not be discoverable from the Tauri runtime environment on some machines
- child-process cleanup on app exit must be handled carefully
- provider secrets now live in the Tauri config store, so future hardening may require OS keychain integration

## Implementation Boundary

This phase ends when the desktop app can:

1. store all required service config in the UI
2. launch both Python services without `runtime.local.yaml`
3. complete the desktop registration path using only app-managed config
