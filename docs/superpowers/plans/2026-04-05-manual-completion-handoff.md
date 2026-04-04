# Manual Completion Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an in-app manual completion handoff flow so users can finish Orchids login and visitor verification in a dedicated Tauri window, then re-check and refresh the account profile from the desktop app.

**Architecture:** Reuse the existing account/profile-refresh pipeline instead of inventing a new completion protocol. Add a small Tauri-side completion window manager plus two account commands, then expose them in the Accounts page with a focused modal workflow.

**Tech Stack:** Tauri 2, Rust, React, TypeScript

---

### Task 1: Add completion window state in Tauri

**Files:**
- Modify: `src-tauri/src/state.rs`
- Test: `src-tauri/src/state.rs`

- [ ] Add an in-memory map/set for active account completion windows keyed by `account_id`.
- [ ] Add methods to register, query, and clear an active completion window.
- [ ] Add unit tests for duplicate registration rejection and cleanup behavior.
- [ ] Run: `cargo test -q`
- [ ] Commit the state-only change.

### Task 2: Add Tauri commands for opening and checking completion

**Files:**
- Modify: `src-tauri/src/commands/accounts.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/accounts.rs`

- [ ] Add `open_account_completion_window(account_id)` command.
- [ ] Validate the account exists and prevent duplicate completion windows for the same account.
- [ ] Create a dedicated webview window labelled from the account id and pointing at the Orchids login/completion URL.
- [ ] Add `check_account_completion(account_id)` command.
- [ ] Reuse `build_profile_session_context` and `fetch_plan_and_credits_with_session`.
- [ ] On success, persist `plan / credits` with `db::update_account_plan_credits` and return the refreshed account row.
- [ ] On failure, return a user-facing error without mutating the account.
- [ ] Add focused tests for:
  - missing account
  - duplicate open request
  - completion check success path
  - completion check failure path
- [ ] Run: `cargo test -q`
- [ ] Commit the backend command change.

### Task 3: Clean up completion window lifecycle

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/accounts.rs`
- Test: `src-tauri/src/commands/accounts.rs`

- [ ] Hook completion-window close events so active window state is cleared when the user closes the window.
- [ ] If a window for the same account already exists, focus it instead of opening a second one.
- [ ] Add or extend tests around active-window cleanup behavior.
- [ ] Run: `cargo test -q`
- [ ] Commit the lifecycle change.

### Task 4: Expose completion commands to the frontend

**Files:**
- Modify: `ui/src/lib/tauri-api.ts`
- Modify: `ui/src/lib/types.ts`

- [ ] Add typed frontend wrappers for `open_account_completion_window` and `check_account_completion`.
- [ ] Add lightweight frontend state types for the modal flow if needed.
- [ ] Run: `npm test -- --runInBand` if a frontend test runner exists; otherwise note no frontend automated test command is configured.
- [ ] Commit the frontend API wiring.

### Task 5: Add the Accounts page completion workflow

**Files:**
- Modify: `ui/src/pages/AccountsPage.tsx`

- [ ] Add a “继续补全” action for accounts where `register_complete` is true and `plan / credits` are still missing.
- [ ] Add a focused modal explaining the manual handoff.
- [ ] Wire modal actions:
  - open completion window
  - check completion
  - close modal
- [ ] On successful check, refresh the local account list and close the modal.
- [ ] On failure, keep the modal open and surface the returned error.
- [ ] If practical, add a page-level test; otherwise cover by manual verification.
- [ ] Run the available frontend verification command.
- [ ] Commit the Accounts page flow.

### Task 6: End-to-end verification

**Files:**
- Review: `docs/superpowers/specs/2026-04-05-manual-completion-handoff-design.md`

- [ ] Run: `cargo test -q`
- [ ] Run the frontend verification command if available.
- [ ] Manually verify:
  - a completion-eligible account shows the new action
  - clicking the action opens the dedicated window
  - duplicate open focuses the existing window
  - manual completion followed by “我已完成，继续检测” updates `plan / credits`
- [ ] Commit the final verified implementation.
