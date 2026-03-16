# OpenClaw Browser Skill Compatibility Design

**Date:** 2026-03-16

**Goal:** Add the minimum OpenClaw-compatible runtime surface required for WorkClaw to run `xiaohongshu-ops-skill` through the "half-publish" flow: open the creator page, upload assets, fill title/body, and stop with the publish button visible.

## Problem

`xiaohongshu-ops-skill` is not just a prompt-only skill. It assumes an OpenClaw-style browser runtime with:

- a single `browser` tool that accepts `action=...`
- `profile="openclaw"` as the default browser profile
- stable tab identities via `targetId`
- profile-backed login persistence
- `browser.upload` support
- OpenClaw-flavored upload path conventions such as `/tmp/openclaw/uploads`

WorkClaw already matches OpenClaw reasonably well on the skill-discovery side:

- runtime projects visible skills into the session workspace
- the system prompt exposes `<available_skills>` entries with explicit `SKILL.md` locations
- the `skill` tool can resolve either `invoke_name` or a concrete `SKILL.md` path

That means the current blocker is not "skills cannot be found." The blocker is that the browser runtime contract does not match what OpenClaw browser-first skills expect.

## Current State

### What already aligns well

- Workspace skill projection under `work_dir/skills/...`
- Explicit skill-path prompt injection
- Session-scoped `skill` invocation
- Existing sidecar bridge pattern for browser tools
- Existing vendored OpenClaw route engine in the sidecar

### What is currently incompatible

- WorkClaw exposes many `browser_*` tools instead of one OpenClaw-style `browser` tool.
- The sidecar browser controller is effectively a single Playwright page, not a profile-aware browser manager.
- `targetId` is surfaced in snapshots but is not backed by real multi-tab state.
- There is no `profiles`, `tabs`, `open`, `focus`, or `upload` action surface matching OpenClaw.
- The current browser session is not durable in the way the skill expects for login reuse.
- Windows command execution uses `cmd /C`, while OpenClaw-authored skills often assume Unix-ish path and copy conventions.

## Design Principles

1. Preserve existing WorkClaw behavior for current users and tests.
2. Add compatibility as a thin, explicit layer instead of replacing the whole runtime.
3. Scope P0 to the `xiaohongshu-ops-skill` half-publish flow.
4. Prefer clear unsupported errors over pretending to support broader OpenClaw features.
5. Keep the browser compatibility layer local-only and WorkClaw-owned; do not add a dependency on an external OpenClaw daemon.

## P0 Scope

P0 covers only the minimum browser/runtime features needed to run `xiaohongshu-ops-skill` to the publish page and stop before the final click.

### In scope

- A unified `browser` tool compatible with OpenClaw-style `action=...` requests
- Session-scoped tool aliases for `read`, `find`, `ls`, and `exec`
- Support for `profile="openclaw"`
- Durable login state for the `openclaw` profile
- Real `targetId` / tab handling for `open`, `tabs`, `focus`, `snapshot`, and `act`
- `browser.upload`
- Windows-safe handling for `/tmp/openclaw/uploads/...`
- System logging for browser compatibility actions and sidecar failures

### Explicitly out of scope for P0

- `profile="chrome"`
- Chrome extension relay
- node browser proxy
- sandbox browser routing
- OpenClaw CLI command compatibility
- generic support for every OpenClaw skill in the ecosystem
- auto-clicking the final publish button

## Architecture

### 1. Session-Scoped Compatibility Tool Surface

WorkClaw should keep all existing `browser_*` tools intact and add a new compatibility surface at session-preparation time:

- `browser`
- `read`
- `find`
- `ls`
- `exec`

The compatibility tools are additive. Existing WorkClaw-native prompts can continue using the current tools, while OpenClaw-authored skills can bind to the names they already expect.

#### Proposed mapping

- `browser` -> new sidecar endpoint accepting OpenClaw-style `action`
- `read` -> alias of `read_file`
- `find` -> alias of `glob`
- `ls` -> alias of `list_dir`
- `exec` -> alias of `bash` for P0

These aliases should be registered during runtime preparation, not as part of the default static registry, so they do not silently alter every current test or non-skill session.

### 2. Unified Browser Compatibility Endpoint

Add a sidecar endpoint dedicated to the compatibility contract, for example:

- `POST /api/browser/compat`

The Rust `browser` tool will forward requests to that endpoint. Internally, the sidecar will dispatch `action` values such as:

- `status`
- `start`
- `profiles`
- `tabs`
- `open`
- `focus`
- `snapshot`
- `act`
- `upload`

This keeps the compatibility logic in one place and avoids spreading OpenClaw-specific request decoding across Rust and TypeScript.

### 3. Persistent `openclaw` Profile

The sidecar must stop treating the browser as a single ephemeral page and instead manage a named browser profile.

P0 needs only one supported durable profile:

- `openclaw`

The sidecar should store that profile under the application data directory, for example:

- `<app_data_dir>/browser/profiles/openclaw/`

The important behavior is persistence, not exact path shape.

Expected outcomes:

- first login to Xiaohongshu can be reused across later runs
- `browser(action="start", profile="openclaw")` is idempotent
- status and profiles calls report something stable and debuggable

Unsupported profiles such as `chrome` should fail fast with a direct error message that says the profile is not available in WorkClaw P0 compatibility mode.

### 4. Tab and `targetId` Model

OpenClaw skills assume that tabs are first-class and that later actions can target the same tab.

P0 therefore needs:

- `open` to return a stable `targetId`
- `tabs` to list active tabs for the profile
- `focus` to switch the active tab
- `snapshot(targetId=...)` to resolve the correct page
- `act(targetId=...)` to run against the correct page

This does not need a fully generic OpenClaw browser service. It only needs a reliable profile-local mapping:

- `profile -> { targetId -> page }`

### 5. Upload Compatibility

`xiaohongshu-ops-skill` expects `browser.upload` and often talks about `/tmp/openclaw/uploads/...`.

P0 should hide this incompatibility behind the compatibility layer instead of asking the model to do path translation itself.

Recommended behavior:

- `browser(action="upload")` accepts ordinary local paths
- if the path already points at `/tmp/openclaw/uploads/...`, map it to a WorkClaw-owned staging directory
- if the path is any other valid local file, stage it internally and upload it

This avoids forcing the agent to run platform-specific copy commands before upload.

### 6. Prompt and Runtime Guidance

The prompt should prefer the compatibility tools for OpenClaw-style skills.

That means:

- `browser` must appear in the available tools list for the session
- alias tools must be visible under OpenClaw-compatible names
- prompt text should not tell the model to use unsupported OpenClaw profiles or relay features

No change is needed to the workspace skill projection model; that part is already close to the OpenClaw design and is not the current blocker.

### 7. Diagnostics

The compatibility layer must write enough detail to system logs to explain failures without letting the model invent a root cause.

Minimum log fields for P0:

- browser action
- requested profile
- resolved targetId
- active tab count
- source upload path
- mapped upload path
- sidecar startup failure cause
- unsupported profile/action errors

## File-Level Design

### Rust / Tauri

- Add a new browser compatibility tool module under `apps/runtime/src-tauri/src/agent/tools/`
- Add a small generic alias-tool module under `apps/runtime/src-tauri/src/agent/tools/`
- Register both during `prepare_runtime_tools(...)`
- Keep the current `browser_*` registration path unchanged

### Sidecar

- Extend `apps/runtime/sidecar/src/browser.ts` from single-page control to profile/tab-aware state
- Add a compatibility API route in `apps/runtime/sidecar/src/index.ts`
- Add or refactor small helpers only where they simplify path staging or tab bookkeeping

### Tests

- Rust integration tests for compatibility tool registration and alias registration
- Sidecar tests for profile persistence, tab routing, upload staging, and compatibility endpoint behavior
- Keep the existing browser-local automation tests and extend them incrementally instead of replacing them

## Risks

### Risk: P0 silently expands into full OpenClaw parity

Mitigation:

- keep P0 action list short
- hard-fail unsupported profiles and relay-specific flows
- do not implement CLI compatibility in this phase

### Risk: Durable profile handling destabilizes current local browser flows

Mitigation:

- keep the new logic behind the compatibility endpoint and `profile="openclaw"`
- leave current `browser_*` endpoints untouched

### Risk: Upload compatibility becomes cross-platform filesystem debt

Mitigation:

- centralize staging logic in one helper
- use app-owned directories
- log both incoming and staged paths

### Risk: Tool name aliases create confusion in non-skill sessions

Mitigation:

- register aliases only in runtime-prepared session registries
- do not add them to `with_standard_tools()`

## Acceptance Criteria

P0 is complete when all of the following are true:

1. An imported `xiaohongshu-ops-skill` session can choose and read the skill from the projected workspace.
2. The model can call `browser(action="start", profile="openclaw")` successfully.
3. Xiaohongshu login survives a restart of the WorkClaw runtime via the durable `openclaw` profile.
4. The model can open the creator page, keep acting on the same `targetId`, and reach the edit screen reliably.
5. The model can upload one or more local images without manual path conversion by the user.
6. The session can stop with the publish button visible and unclicked.
7. On failure, system logs show the actual sidecar/browser compatibility failure reason instead of only a timeout shell.

## Recommended Next Step

Implement P0 as a compatibility layer for `xiaohongshu-ops-skill` first, then reassess whether the resulting browser surface is broad enough to onboard additional OpenClaw browser-first skills without further architectural change.
