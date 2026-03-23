# Rust AI Guardrails Design

**Goal:** Establish repository guidance for AI-native Rust development in `apps/runtime/src-tauri/` before doing broad codebase refactors.

## Why This Comes First

WorkClaw's Rust runtime has several files above 1000 lines, with a few command modules already above 3000 to 6000 lines. The problem is not only historical code shape. In AI-assisted development, large files keep growing when the repository does not give the coding agent a clear module map, file-budget policy, and preferred landing zones for new logic.

This design treats guidance files as the first control layer. The immediate goal is to make Codex and Claude Code choose better file boundaries before touching runtime behavior.

## Research-Based Policy

The proposed thresholds are governance triggers, not absolute quality scores.

- `<= 500` lines: normal target zone for runtime files
- `501-800` lines: warning zone; avoid adding net-new business logic until module placement is reconsidered
- `801+` lines: split-design zone; write or update a split plan before implementing new feature work in that file

This is intentionally simpler than a four-threshold model. The purpose is to prevent giant-file growth without forcing the repo into many tiny files.

## Anti-Goals

- Do not require immediate DDD migration of all Rust code
- Do not block small bug fixes in large files when fast repair is safer
- Do not force micro-files for trivial helpers
- Do not rewrite existing command contracts just to satisfy folder purity

## Recommended Rust Layering

For Tauri runtime work, the default landing zones should be:

- `commands/*.rs`: Tauri entrypoints, request parsing, response shaping, and orchestration handoff
- `commands/<domain>/service.rs`: business rules and use-case orchestration
- `commands/<domain>/repo.rs`: SQLite reads and writes
- `commands/<domain>/gateway.rs` or `adapter.rs`: external system interaction
- `commands/<domain>/types.rs`: internal request/result/domain DTOs when they no longer fit naturally in the root command file

This is intentionally pragmatic rather than pure DDD. It matches the current repository direction and can later evolve into stronger domain folders if needed.

## Responsibility Rules

### Command Layer

- own Tauri command entrypoints, input parsing, response shaping, and handoff to deeper layers
- keep Tauri command signatures and frontend-facing payloads stable unless the task explicitly changes the contract
- do not own long SQL blocks or multi-step business rules

### Service Layer

- own business rules, validation, normalization, and multi-step orchestration
- decide how repositories and gateways are combined for a use case
- do not own raw SQL or protocol-specific plumbing

### Repository Layer

- own SQLite queries, writes, transactions, and row mapping
- keep persistence details out of commands and services
- do not own business policy decisions

### Gateway Or Adapter Layer

- own external system calls, provider APIs, and protocol translation
- keep platform-specific integration details out of commands, services, and repos

## File Count Guardrail

To avoid replacing giant files with noisy micro-files, new files should meet at least one of these conditions:

- they hold a separate persistence concern
- they hold a separate external integration concern
- they hold a distinct business use case
- extracting them removes meaningful branching or data-shaping complexity from a larger file

Avoid creating one-file-per-function helper directories.

## Guidance File Layout

The repository should use two levels of guidance:

1. Root `AGENTS.md`
   - short cross-repo rules
   - points Rust work to the Tauri-specific guidance
   - does not restate every Rust detail

2. `apps/runtime/src-tauri/AGENTS.md`
   - Rust-runtime-specific module placement rules
   - file-budget rules
   - command/service/repo/gateway responsibility split
   - verification reminders for runtime and SQLite-sensitive changes

Detailed explanations belong in docs, not in giant agent memory files.

## First-Round Rules To Enforce In Guidance

- New Rust runtime work must name the target layer before coding.
- New business rules should not be added directly to files above 500 lines unless the change is a narrow repair.
- Files above 800 lines require a short split plan before adding feature work.
- Commands should hand off non-trivial business logic to service/repo/gateway modules.
- SQLite-sensitive read-path changes must preserve legacy compatibility or ship with a migration and regression test.
- Runtime refactors should preserve existing user-visible behavior unless the change is intentional and called out.

## Future Automation, But Not In Phase 1

The first phase is documentation and developer guidance only. Future phases may add:

- line-count reporting scripts
- dependency-direction checks
- command-file warnings in CI
- PR/review checklist automation

## Success Criteria

- Root guidance stays concise and points to Rust-specific rules
- Rust runtime work has a local `AGENTS.md` with explicit file placement guidance
- The repository documents the `500 / 800` thresholds as governance triggers
- Agents are told how to avoid both giant files and micro-file sprawl
