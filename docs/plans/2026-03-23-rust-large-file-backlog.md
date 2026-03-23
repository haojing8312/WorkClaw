# Rust Large File Backlog

**Goal:** Turn the current `500 / 800` Rust runtime guardrails into an actionable backlog for `apps/runtime/src-tauri/`.

**Source:** Generated from `node scripts/report-rust-large-files.mjs` on 2026-03-23 using thresholds `warn=500` and `plan=800`.

## Prioritization Rules

- Prioritize `commands/*.rs` files first because they are the highest-risk place for AI-assisted feature accretion.
- Prioritize startup-critical and runtime-core files second because they affect broad behavior and are expensive to verify after every change.
- Prioritize tests and tools third unless they are actively blocking refactors.
- Large child modules created during recent splits can stay in backlog, but they should not restart the pattern of becoming new giant files.

## Priority 1: Giant Command Entrypoints

These files should be the first ongoing refactor targets. New feature work in them should start with a split plan.

### P1-A: Plugin and integration command surfaces

- `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs` — 6628 lines
  - Why first: largest command file in the Rust runtime and likely a magnet for continued feature growth
  - First split direction: separate command entrypoints from plugin service logic, plugin repo/persistence logic, and provider or host integration glue
  - First safe step: inventory command functions and group them by use case before moving any code

- `apps/runtime/src-tauri/src/commands/feishu_gateway.rs` — 3545 lines
  - Why first: external integration plus runtime orchestration mixed in one file
  - First split direction: keep command shell, move protocol/integration logic into `gateway` or `adapter`, move persistence into `repo`, move business flow into `service`
  - First safe step: isolate external Feishu API and protocol handling from local runtime business rules

- `apps/runtime/src-tauri/src/commands/clawhub.rs` — 2469 lines
  - Why first: likely combines command handling, remote integration, and local state updates
  - First split direction: command shell plus `service` for business flow plus `gateway` for Clawhub interactions
  - First safe step: identify remote call boundaries and move them behind a gateway module

### P1-B: Chat runtime command surfaces

- `apps/runtime/src-tauri/src/commands/chat_runtime_io.rs` — 2118 lines
  - Why first: runtime I/O paths tend to attract broad changes and regressions
  - First split direction: keep transport and command surface thin, move stateful orchestration and persistence into submodules
  - First safe step: separate read/write persistence helpers from session or runtime flow orchestration

- `apps/runtime/src-tauri/src/commands/chat_session_io.rs` — 2048 lines
  - Why first: startup-critical and persistence-heavy
  - First split direction: `repo` for session storage plus `service` for session lifecycle rules
  - First safe step: isolate raw SQLite access from session selection and lifecycle behavior

## Priority 2: Runtime Core And Infrastructure

These files are not command entrypoints, but they are large enough to deserve planned thinning.

- `apps/runtime/src-tauri/src/agent/executor.rs` — 1919 lines
  - Why here: central runtime engine logic with broad blast radius
  - First split direction: carve out pure orchestration helpers, policy helpers, and tool-execution adapters
  - First safe step: continue earlier refactor direction that moves reusable policy and pure logic into smaller boundaries

- `apps/runtime/src-tauri/src/db.rs` — 1146 lines
  - Why here: schema and persistence helpers are likely shared widely
  - First split direction: separate connection/bootstrap helpers from schema migration and query utility helpers
  - First safe step: identify pieces that can move without changing database call sites

## Completed Template

- `apps/runtime/src-tauri/src/commands/employee_agents.rs` — now 799 lines
  - Status: completed as the first formal Rust splitting template
  - Outcome: root file is now below the `800` split-design threshold
  - Follow-up: reuse this module structure as the reference pattern for later command-file governance

## Priority 3: Large Child Modules And Tooling

These files are already submodules or test/tool files. They should not be ignored, but they do not need to lead the queue unless they block active work.

- `apps/runtime/src-tauri/src/commands/employee_agents/repo.rs` — 2079 lines
  - Risk: recent split work may just be moving the giant-file problem into the child layer
  - First split direction: split by persistence concern rather than by generic helper extraction

- `apps/runtime/src-tauri/src/commands/employee_agents/service.rs` — 1748 lines
  - Risk: business orchestration is still too concentrated
  - First split direction: split by use case such as listing, upsert, delete, association, or reconciliation flow

- `apps/runtime/src-tauri/src/agent/tools/employee_manage.rs` — 988 lines
  - Risk: tool logic can accrete mixed validation, orchestration, and formatting concerns
  - First split direction: separate tool entrypoint glue from underlying use-case helpers

- `apps/runtime/src-tauri/tests/test_im_employee_agents.rs` — 3612 lines
  - Risk: hard to evolve and diagnose failures
  - First split direction: split by scenario family or legacy-compatibility lane
  - Note: tests are lower priority than production command files unless they block refactors

## Current Warn Queue

These files are above 500 lines and should be watched, but they are not first in line for dedicated split work.

- `apps/runtime/src-tauri/src/adapters/openai.rs` — 756 lines
- `apps/runtime/src-tauri/src/commands/models_repo.rs` — 746 lines
- `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs` — 666 lines
- `apps/runtime/src-tauri/tests/helpers/mod.rs` — 658 lines
- `apps/runtime/src-tauri/src/commands/skills.rs` — 643 lines
- `apps/runtime/src-tauri/src/commands/runtime_preferences.rs` — 637 lines
- `apps/runtime/src-tauri/src/lib.rs` — 614 lines
- `apps/runtime/src-tauri/src/commands/packaging.rs` — 613 lines
- `apps/runtime/src-tauri/tests/test_session_export_recovery.rs` — 609 lines
- `apps/runtime/src-tauri/src/agent/run_guard.rs` — 597 lines
- `apps/runtime/src-tauri/tests/test_e2e_flow.rs` — 583 lines
- `apps/runtime/src-tauri/src/diagnostics.rs` — 562 lines
- `apps/runtime/src-tauri/src/commands/models.rs` — 539 lines
- `apps/runtime/src-tauri/tests/test_feishu_gateway.rs` — 529 lines
- `apps/runtime/src-tauri/src/team_templates.rs` — 514 lines
- `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs` — 513 lines
- `apps/runtime/src-tauri/src/commands/chat_policy.rs` — 506 lines
- `apps/runtime/src-tauri/tests/test_approval_bus.rs` — 505 lines

## Recommended Execution Order

1. Use `employee_agents` as the formal reference template for future command splitting.
2. Tackle `feishu_gateway.rs` next because it mixes external integration with runtime behavior and is now the most natural follow-on target.
3. Tackle `openclaw_plugins.rs` as a dedicated multi-step effort because of its sheer size.
4. Then move to `clawhub.rs`, `chat_runtime_io.rs`, and `chat_session_io.rs`.
5. Re-run `node scripts/report-rust-large-files.mjs` after each split milestone and update this backlog rather than treating it as static.

## Definition Of Backlog Progress

- A file leaves the `PLAN` queue only when it falls below 800 lines or when remaining content is clearly limited to a single responsibility.
- A file is not considered improved if code was merely moved into an equally giant child file.
- A split is successful only when command, service, repo, and gateway boundaries are clearer after the change than before it.
