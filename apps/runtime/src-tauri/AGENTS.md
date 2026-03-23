# Rust Runtime AGENTS.md

## Scope
- This file applies to work in `apps/runtime/src-tauri/`.
- Use it as the local Rust runtime guidance layer on top of the root `AGENTS.md`.

## Primary Goal
- Keep Tauri runtime changes maintainable during AI-native development.
- Prefer clear module boundaries over continuing to grow giant command files.
- Avoid replacing giant files with many trivial micro-files.

## Default Landing Zones
- `src/commands/*.rs`: Tauri command entrypoints, request parsing, response shaping, and handoff to deeper layers
- `src/commands/<domain>/service.rs`: business rules and use-case orchestration
- `src/commands/<domain>/repo.rs`: SQLite reads, writes, and row mapping
- `src/commands/<domain>/gateway.rs` or `adapter.rs`: external system or provider integration
- `src/commands/<domain>/types.rs`: internal DTOs and helper types once the root command file becomes crowded

When a task does not naturally fit these landing zones, explain the chosen placement before editing code.

## Current Reference Template
- Treat `src/commands/employee_agents.rs` plus the sibling directory `src/commands/employee_agents/` as the current Rust-side reference template for large command-file governance.
- Use that module as the first place to copy structure from before inventing a new split pattern.
- The current reference layout includes:
  - `types.rs` for DTOs and helper types
  - `profile_service.rs` and `profile_repo.rs` for employee CRUD
  - `feishu_service.rs` for employee/Feishu association orchestration
  - `routing_service.rs` and `session_service.rs` for employee routing and session flows
  - `group_management.rs` for team/group create, clone, list, and delete logic
  - `group_run_service.rs`, `group_run_snapshot_service.rs`, `group_run_action_service.rs`, and `group_run_entry.rs` for group-run state, read, action, and entry flow
  - `memory_commands.rs` and `tauri_commands.rs` for command implementation bodies that the root file wraps
- Prefer matching this shape when splitting the next giant Rust command surface unless there is a clear reason not to.

## Responsibility Split
- Commands own Tauri entrypoints, input parsing, response shaping, and handoff to deeper layers.
- Services own business rules, validation, normalization, and multi-step orchestration.
- Repositories own SQLite queries, writes, transactions, and row mapping.
- Gateways/adapters own external system calls, provider APIs, and protocol translation.
- Keep existing Tauri command names and payload contracts stable unless the task explicitly changes them.

## File Budget Policy
- `<= 500` lines: target zone for Rust runtime files
- `501-800` lines: warning zone; avoid adding net-new business logic until module placement is reconsidered
- `801+` lines: split-design zone; write or update a short split plan before adding feature work

These thresholds are governance triggers, not blanket failure rules. Do not split files mechanically just to get under a number.

`employee_agents.rs` has already been reduced below the `800` split-design threshold and should now be treated as a maintained sample of the intended end state.

## Avoid Micro-File Sprawl
- Create a new file only when it owns a real persistence concern, integration concern, or distinct use case.
- Do not create one-file-per-helper directories for trivial logic.
- Prefer extracting cohesive chunks that remove meaningful branching, SQL, or protocol handling from a larger file.

## SQLite And Compatibility Rules
- Any new query dependency on schema shape must keep backward compatibility through a migration or a legacy-schema fallback.
- For startup-critical reads, session lists, IM bindings, and similar paths, add or preserve regression coverage for legacy schema behavior.

## Verification Reminders
- Use `$workclaw-implementation-strategy` before changing runtime behavior, routing, provider integration, tool permissions, sidecar protocols, IM orchestration behavior, or vendor sync boundaries.
- Use `$workclaw-change-verification` before claiming Rust runtime work is complete.
- Prefer the smallest verification command set that proves the touched Rust surface, but do not skip required checks.

## Working Style For AI Agents
- Name the intended target layer before writing new Rust runtime logic.
- If touching a file above 500 lines, explain why the change belongs there instead of a new module.
- If touching a file above 800 lines for feature work, create or update a split plan in `docs/plans/` first.
- Preserve visible behavior unless the task explicitly calls for a behavior change.
