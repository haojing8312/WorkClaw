# Rust Runtime Preferences Split Design

**Goal:** Turn `apps/runtime/src-tauri/src/commands/runtime_preferences.rs` into a thinner Rust command module by separating preference DTOs, app-setting persistence, normalization/service logic, and OS autostart handling without changing the existing Tauri command contract.

## Why This Split

`runtime_preferences.rs` is a medium-sized command file that mixes four different concerns:

- Tauri command wrappers
- preference DTOs and constants
- SQLite-backed app-setting reads and writes
- OS autostart synchronization for Windows, macOS, and Linux

That mix is manageable today, but it is exactly the kind of file that tends to absorb unrelated preference flags over time. Splitting it now keeps the command shell thin before it becomes another giant helper file.

## Recommended Module Layout

Create these child modules under `apps/runtime/src-tauri/src/commands/runtime_preferences/`:

- `types.rs`
  - owns `RuntimePreferences`, `RuntimePreferencesInput`, preference keys, and default constants
- `repo.rs`
  - owns `get_app_setting` and `set_app_setting`
- `service.rs`
  - owns `get_runtime_preferences_with_pool`, `set_runtime_preferences_with_pool`, and `resolve_default_work_dir_with_pool`
  - owns path and value normalization helpers
- `autostart.rs`
  - owns `sync_launch_at_login` and platform-specific autostart helpers

Keep `runtime_preferences.rs` as the command entrypoint and stable public surface.

## Boundary Rules

- `types.rs` should not query the database or touch the filesystem.
- `repo.rs` should only read/write `app_settings`.
- `service.rs` should own runtime preference normalization and application logic.
- `autostart.rs` should own OS-specific startup registration logic and any helper functions it needs.
- The root file should only keep `#[tauri::command]` wrappers, minimal compatibility glue, and test coverage that is easier to keep colocated.

## Compatibility Rules

- Preserve existing command names and response payloads.
- Preserve current default values:
  - `default_language = zh-CN`
  - `immersive_translation_enabled = true`
  - `immersive_translation_display = translated_only`
  - `immersive_translation_trigger = auto`
  - `translation_engine = model_then_free`
  - `launch_at_login = false`
  - `launch_minimized = false`
  - `close_to_tray = true`
  - `operation_permission_mode = standard`
- Keep `set_runtime_preferences` behavior the same, including the post-save autostart sync call.

## Test Strategy

Preserve the current runtime preference tests and add focused coverage only if the first split exposes an uncovered behavior. The existing tests already cover:

- default preference derivation
- round-trip save and reload
- partial update semantics
- invalid permission mode fallback

## Success Criteria

- The root file becomes a thin command shell.
- Preference constants and DTOs are isolated from service logic.
- App-setting SQL access is isolated from normalization logic.
- Autostart sync remains behaviorally unchanged.
- `pnpm test:rust-fast` stays green.
