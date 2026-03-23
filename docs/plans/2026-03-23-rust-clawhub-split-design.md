# Rust ClawHub Split Design

**Goal:** Turn [clawhub.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/clawhub.rs) into the next formal Rust command-splitting target after `employee_agents`, using the same large-file governance pattern while preserving the current ClawHub, SkillHub, and translation-facing contracts.

## Current Status

This split is now in progress.

- Root [clawhub.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/clawhub.rs) has already started shrinking and is now down to about `266` lines from `2469`
- The first batch extracted `types.rs`, `support.rs`, and `repo.rs`
- The second batch extracted `search_service.rs` and `detail_service.rs`
- The third batch extracted `translation_service.rs`
- The fourth batch extracted `download_service.rs`
- The fifth batch extracted `install_service.rs`
- The root file is now effectively a thin command shell plus a few shared helpers
- The current safest follow-on cut is still to keep extracting by concern boundary instead of widening the existing helper modules

## Strategy Summary
- Change surface: `apps/runtime/src-tauri/src/commands/clawhub.rs` and adjacent clawhub tests
- Affected modules: catalog search/list/detail, sync/cache helpers, translation helpers, repo download/install/update, Tauri command wrappers
- Main risk: breaking user-visible ClawHub catalog search, SkillHub sync, translation, or installation behavior while moving logic into child modules
- Recommended smallest safe path: start with `types.rs` plus a narrow pure-helper module, keep the root file as the visible command shell, and avoid creating a giant replacement child module on day one
- Required verification: focused clawhub tests for library cache, search, index sync, and the Rust fast path
- Release impact: low if command contracts stay stable, but user-visible runtime behavior means regressions would be customer-facing

## Why This Is Next

[clawhub.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/clawhub.rs) is one of the clearest remaining giant command surfaces because it combines several unrelated concerns in one place:

- SkillHub and ClawHub catalog URL construction
- catalog item and detail normalization
- query scoring and recommendation shaping
- cache key generation and cache age decisions
- GitHub/SkillHub archive download helpers
- translation helpers and model fallback logic
- repo install/update orchestration
- Tauri command entrypoints and test helpers

That makes it a strong candidate for the same governance pattern used for `employee_agents` and `feishu_gateway`: a thin root command file plus focused child modules.

## Current Problem

The file currently mixes these concern groups:

- public DTOs and small internal row structs
- low-level URL and cache key helpers
- catalog search and recommendation normalization
- HTTP cache reads and writes
- SkillHub sync and local page fallback
- detail lookup and repo URL resolution
- translation engine selection and fallback
- GitHub archive download / extraction / install / update
- Tauri command wrappers and unit tests

That creates three maintenance issues:

1. unrelated edits drag too much context into one file
2. pure normalization logic is hard to reuse cleanly because it sits next to command wrappers
3. AI-native feature work is too likely to pile into the root file because it is the easiest visible landing zone

## Recommended Design

### 1. Keep the root file as the visible command shell

The root [clawhub.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/clawhub.rs) should keep:

- public Tauri command wrappers
- compatibility glue needed by current callers
- a small amount of orchestration that truly belongs at the command entrypoint

It should stop owning all DTO definitions and pure helper logic.

### 2. Split by responsibility, not helper count

Use focused child modules under `apps/runtime/src-tauri/src/commands/clawhub/`:

- `types.rs`
  - public DTOs and small internal row structs
- `support.rs`
  - pure URL, cache-key, slug, cursor, and catalog normalization helpers
- `repo.rs`
  - HTTP cache reads/writes and SkillHub index persistence helpers
- `search_service.rs`
  - search, recommendation, and result shaping logic
- `detail_service.rs`
  - skill detail lookup, repo URL resolution, and detail cache orchestration
- `translation_service.rs`
  - language normalization and text translation orchestration
- `download_service.rs`
  - GitHub / SkillHub archive download and extraction helpers
- `install_service.rs`
  - install and update orchestration
- `tauri_commands.rs`
  - concrete command implementations that the root file wraps

### 3. Preserve current behavior seams

The split should keep these seams stable:

- existing Tauri command names and payload shapes
- current cache-key formats
- current SkillHub pagination behavior
- current translation fallback behavior
- current GitHub fallback and ClawHub fallback behavior for downloads
- current install/update behavior

This should be an internal structure refactor first, not a behavior redesign.

## Suggested First-Cut Order

1. `types.rs`
2. `support.rs`
3. `repo.rs`
4. `search_service.rs`
5. `detail_service.rs`
6. `translation_service.rs`
7. `download_service.rs`
8. `install_service.rs`
9. `tauri_commands.rs`

## What Not To Do

- do not replace one giant root file with one giant helper file
- do not split purely by “small helper” without a real concern boundary
- do not move Tauri macros into child modules and rely only on re-export if that breaks macro visibility expectations
- do not couple cache persistence, translation, and download flow in the same child file

## Recommended End State

The target shape should look like this:

- root `clawhub.rs` is a thin shell
- public DTOs live in `types.rs`
- pure catalog helpers live in `support.rs`
- persistence moves into `repo.rs`
- search/detail/translation/download/install each get a focused home
- tests no longer bloat the root file itself

## Success Criteria

- [clawhub.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/clawhub.rs) becomes materially smaller
- no single replacement child file becomes the new giant dumping ground
- existing Tauri commands and payload contracts remain stable
- existing ClawHub and SkillHub tests stay green
- the split pattern remains recognizably aligned with the `employee_agents` and `feishu_gateway` reference templates
