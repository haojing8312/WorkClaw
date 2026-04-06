# WorkClaw Single Runtime Root Design

**Date:** 2026-04-06

## Goal

Unify WorkClaw desktop runtime data under one user-configurable root directory so database files, diagnostics, cache, plugin state, session journals, and the default workspace all move together and can be migrated safely.

## Scope

This design covers the desktop runtime path model, startup bootstrap, runtime-root migration, and the desktop settings UI.

Included:

- one user-visible `WorkClaw` runtime root directory
- startup bootstrap outside the main runtime database
- automatic migration from legacy system directories into the new root model
- user-triggered migration to a new root directory with restart
- rollback behavior when migration fails
- desktop settings simplification to one path entry

Excluded for now:

- portable / ZIP install mode
- Linux or macOS specific UX polish beyond keeping the path model cross-platform-friendly
- per-feature opt-out storage locations
- replacing per-session workspace overrides inside chat

## Problem Summary

The current desktop app exposes several directory concepts at once:

- application data directory
- cache directory
- log directory
- diagnostics directory
- default work directory

Those paths are sourced from different Tauri system APIs and one runtime preference, so the UI asks the user to reason about several storage concepts that should really move together.

This has three user-visible problems:

1. Users need to understand too many directories for one product.
2. Changing the effective storage location is awkward because runtime data is split across multiple system directories.
3. The current `default_work_dir` is configured independently, so it does not naturally follow the rest of the runtime data.

The requested target behavior is closer to Codex: one root directory, one user decision, and everything else derived below that root.

## Root Causes

### 1. Runtime state is anchored to several unrelated system directories

The Rust runtime currently reads paths from:

- `app.path().app_data_dir()`
- `app.path().app_cache_dir()`
- `app.path().app_log_dir()`

Different subsystems then append their own child paths. That means runtime data does not share one authoritative root.

### 2. Root location is not bootstrapable outside the database

The current `default_work_dir` lives in `app_settings`, but the database itself lives under the application data directory. If the database must move with the runtime root, the root cannot depend on the database to discover itself.

### 3. Settings UI mirrors implementation details

`DesktopSettingsSection` currently shows raw implementation paths and exposes them directly. That leaks internal storage layout to end users and increases mental overhead without offering meaningful control.

## Target Architecture

### 1. Introduce one authoritative runtime root

WorkClaw should resolve one authoritative root directory for all desktop runtime data.

Recommended Windows default:

- `%USERPROFILE%\\.workclaw`

Recommended derived layout:

- `<root>\\workclaw.db`
- `<root>\\workclaw.db-wal`
- `<root>\\workclaw.db-shm`
- `<root>\\workspace\\`
- `<root>\\sessions\\`
- `<root>\\diagnostics\\`
- `<root>\\cache\\`
- `<root>\\openclaw-plugins\\`
- `<root>\\openclaw-state\\`
- `<root>\\openclaw-cli-shim\\`
- `<root>\\skills\\vendor\\`

The user-facing mental model becomes:

- choose one WorkClaw data directory
- everything else lives under it

### 2. Add a bootstrap configuration file outside the migrated root

The runtime needs a small stable bootstrap file stored in a system-managed location that remains discoverable before opening the main database.

Recommended location on Windows:

- `%APPDATA%\\dev.workclaw.runtime\\bootstrap-root.json`

Recommended responsibilities:

- store `current_root`
- store any `pending_migration`
- record `previous_root`
- record `last_migration_result`
- store a small schema version for future bootstrap evolution

This file should be tiny, transactional, and independent of the main runtime database.

### 3. Centralize path derivation in one runtime-path module

Create a dedicated Rust module, for example `runtime_paths.rs`, that owns:

- default root resolution
- bootstrap file resolution
- derived child-path generation
- legacy-path discovery for upgrade compatibility
- path validation helpers

All runtime code that currently uses `app_data_dir`, `app_cache_dir`, or `app_log_dir` directly should move to this shared path model.

### 4. Run migrations before runtime subsystems initialize

Migration must happen before these subsystems open files:

- diagnostics
- SQLite pool
- session journal store
- plugin state and shim state
- plugin host workspaces
- any startup snapshot or audit writer

That means startup should become:

1. load bootstrap
2. resolve effective root
3. complete or recover any pending migration
4. build runtime paths
5. initialize diagnostics, database, and remaining runtime state

### 5. Make default workspace a derived path

The global default workspace should stop being a separately user-edited directory in desktop settings.

New rule:

- global default workspace = `<root>\\workspace`

Per-session workspace overrides remain allowed in chat creation or session editing, but the global default is derived, not independently configured.

## Bootstrap And Migration Model

### Bootstrap schema

Suggested shape:

```json
{
  "schema_version": 1,
  "current_root": "C:\\Users\\me\\.workclaw",
  "pending_migration": {
    "from_root": "C:\\Users\\me\\AppData\\Roaming\\dev.workclaw.runtime",
    "to_root": "D:\\WorkClawData",
    "status": "pending",
    "created_at": "2026-04-06T10:00:00Z",
    "last_error": null
  },
  "previous_root": null,
  "last_migration_result": null
}
```

Suggested migration statuses:

- `pending`
- `in_progress`
- `failed`
- `completed`
- `rolled_back`

### Migration stages

The runtime-root migration should behave like a small startup transaction.

Recommended flow:

1. User selects a new root in settings.
2. Runtime validates the target path and writes `pending_migration` to bootstrap.
3. Runtime restarts.
4. Startup sees `pending_migration` before database initialization.
5. Migration copies or moves all managed paths to the target root.
6. Startup validates the new root.
7. If validation passes, bootstrap switches `current_root` to the new root and stores the old one as `previous_root`.
8. Runtime starts using the new root.
9. Old root cleanup only happens after a later confirmed clean exit.

### Managed path set

Migration should operate on one centralized managed-path specification, not ad-hoc copy logic in individual modules.

Managed items should include:

- database files
- diagnostics tree
- cache tree
- session journals
- plugin workspace and state directories
- shim state
- vendored builtin skill directory

If future runtime features add new root-owned directories, they should extend this central spec so migrations stay complete.

### Same-volume vs cross-volume behavior

Recommended behavior:

- same volume: prefer move / rename where safe
- cross volume: use recursive copy, then verify, then switch

In both cases:

- do not delete the old root before validation
- keep rollback metadata in bootstrap
- avoid partial switch-over without validation

## Failure Recovery

Failure handling should prefer safe fallback over partial adoption.

### Failure rules

- If the target root fails validation before cutover, keep using the old root.
- If startup sees an interrupted `in_progress` migration, recover by restoring the old root as current.
- If the new root becomes active but a startup validation check fails, mark the migration failed and restore the old root in bootstrap.
- If the runtime starts successfully on the new root, old-root cleanup remains deferred until a confirmed later clean exit.

### User-visible recovery

Settings should surface:

- last migration target
- whether the last migration succeeded or failed
- concise error summary on failure

This keeps failures visible without forcing users to inspect raw filesystem details.

## Legacy Compatibility Strategy

The new model must also support upgrade from the pre-bootstrap layout.

### Upgrade discovery order

Recommended order:

1. If bootstrap exists, trust it.
2. If bootstrap does not exist but legacy WorkClaw data exists in old system directories, synthesize bootstrap using those legacy locations as the current active root-equivalent source.
3. If neither exists, initialize a fresh bootstrap pointing at the default runtime root.

This means an upgrade does not force an immediate move. The runtime can first adopt the bootstrap model while still pointing at legacy data, then later migrate on demand.

### Temporary compatibility aliases

During implementation, some code may still think in terms of `app_data_dir`, `app_cache_dir`, or `app_log_dir`.

Compatibility mapping should become logical aliases only:

- legacy app data => `<root>`
- legacy cache => `<root>\\cache`
- legacy logs => `<root>\\diagnostics\\logs`

This preserves internal intent while converging all physical data onto the single root.

## Settings UX

### Visible directory controls

`Settings -> Desktop / System` should show one storage card only:

- `WorkClaw 数据根目录`

Visible actions:

- `选择目录`
- `打开目录`
- `迁移并重启` when the selected value differs from the active root

The settings page should no longer show:

- application data directory
- cache directory
- log directory
- diagnostics directory
- default work directory

### Suggested copy

Primary description:

- `数据库、缓存、日志、诊断、插件状态和会话记录都会保存在这个目录下。`

Secondary description:

- `默认工作目录会自动使用该目录下的 workspace 子目录。`

Restart note:

- `修改后需要重启应用，WorkClaw 会在启动时自动迁移旧数据。`

### Interaction flow

1. User clicks `选择目录`.
2. User picks a directory.
3. If unchanged, do nothing.
4. If changed, show confirmation explaining migration coverage, restart, and rollback safety.
5. On confirm, backend schedules migration in bootstrap.
6. Frontend requests app restart.
7. On next launch, migration runs automatically.

### Validation rules

Reject before scheduling if:

- the target is empty
- the target is not writable
- the target is inside the current root
- the current root is inside the target
- another migration is already pending

### Post-restart feedback

Display a lightweight banner when available:

- success: migrated to new root
- failure: migration failed and WorkClaw stayed on the previous root

Detailed status can be hidden behind a collapsed “view migration details” area.

## Affected Surfaces

- `apps/runtime/src-tauri/src/lib.rs`
- `apps/runtime/src-tauri/src/db.rs`
- `apps/runtime/src-tauri/src/diagnostics.rs`
- `apps/runtime/src-tauri/src/session_journal.rs`
- `apps/runtime/src-tauri/src/commands/runtime_preferences/*`
- `apps/runtime/src-tauri/src/commands/desktop_lifecycle*`
- `apps/runtime/src-tauri/src/commands/openclaw_plugins/*`
- runtime modules that currently call `app_data_dir`, `app_cache_dir`, or `app_log_dir`
- `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- desktop settings tests in `apps/runtime/src/components/__tests__/`

## Risks

### Startup ordering risk

This change touches startup flow before diagnostics and database initialization. A weak sequencing change could break app boot entirely.

### Data loss risk

Runtime-root migration touches SQLite files, plugin state, and journal files. Incomplete verification or premature deletion of the old root could lose user data.

### Legacy compatibility risk

Older installations may have partial data spread across `app_data_dir`, `app_cache_dir`, and `app_log_dir`. The upgrade detector must not silently ignore one of those surfaces.

### UX risk

If the settings flow over-explains implementation details, the UI regresses back into the same complexity the redesign is meant to remove.

## Success Criteria

1. Users only need to understand and configure one WorkClaw data directory.
2. Database, diagnostics, cache, plugin state, and session journals all resolve beneath the same effective root.
3. Global default workspace is derived automatically as `<root>\\workspace`.
4. Legacy installations can upgrade into the bootstrap model without data loss.
5. User-triggered root migration happens through restart-time migration with rollback protection.
6. Settings communicates migration state clearly without exposing internal directory sprawl.

## Verification Expectations

Implementation should include:

- Rust tests for bootstrap discovery and root derivation
- Rust tests for migration scheduling, execution, validation, and rollback
- startup compatibility tests covering legacy-layout adoption
- frontend tests proving the desktop settings page only exposes one root directory control
- `pnpm test:rust-fast` for runtime changes
- targeted frontend tests for desktop settings behavior

## Release Impact

This is a high-sensitivity desktop runtime behavior change. It does not directly alter versioning or installer branding, but it changes where user data lives and how upgrades behave, so it should be treated as a storage- and compatibility-sensitive release item.
