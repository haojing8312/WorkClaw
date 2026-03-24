# Rust OpenClaw Plugins Split Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs` into a thin compatibility shell by extracting plugin inventory, plugin-host inspection, Feishu setup/settings, Feishu runtime supervision, and `openclaw-lark` installer session logic into focused child modules without changing current Tauri contracts.

**Architecture:** Keep the root `openclaw_plugins.rs` file as the macro-visible shell and public re-export layer. Move implementation into child modules under `apps/runtime/src-tauri/src/commands/openclaw_plugins/`, using real execution boundaries instead of one generic `service.rs`, and preserve the existing imports from `lib.rs`, `feishu_gateway.rs`, and `test_feishu_gateway.rs`.

**Tech Stack:** Rust, Tauri commands, sqlx, SQLite, reqwest, std::process, std::sync, WorkClaw runtime tests

---

## Guardrails

- Preserve all current Tauri command names and return payloads.
- Preserve `OpenClawPluginFeishuRuntimeState` and `OpenClawLarkInstallerSessionState` availability from the root module through stable `pub use` exports.
- Do not change `lib.rs` command registration unless the root command names themselves change, which they should not in this split.
- Prefer re-exports from the root module over cross-file import churn in `feishu_gateway.rs`.
- Keep runtime supervision and installer-session process control out of the early tasks; land low-risk deterministic slices first.
- Move tests out of the root file only after the target logic has already been extracted and stabilized.

## Task 1: Create the module skeleton and extract shared types

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/types.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/tests/test_feishu_gateway.rs`

**Step 1: Add the child-module skeleton**

- Add `#[path = "openclaw_plugins/types.rs"] mod types;` to the root file.
- Add placeholder declarations for the later modules:
  - `install_repo.rs`
  - `install_service.rs`
  - `plugin_host_service.rs`
  - `settings_service.rs`
  - `setup_service.rs`
  - `runtime_state.rs`
  - `runtime_bridge.rs`
  - `installer_session.rs`
  - `tauri_commands.rs`
  - `tests.rs`
- Keep all behavior in the root file at this stage.

**Step 2: Move shared DTOs and status structs**

- Move the data-only types into `types.rs`, including:
  - plugin install input and record DTOs
  - inspection result DTOs
  - channel host and snapshot DTOs
  - Feishu runtime status and outbound request/result DTOs
  - setup/settings DTOs
  - installer session DTOs
- Re-export them from the root file.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib derive_channel_capabilities_flattens_runtime_flags -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway parse_feishu_payload_supports_challenge -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/types.rs
git commit -m "refactor(runtime): extract openclaw plugin shared types"
```

## Task 2: Extract install persistence into `install_repo.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/install_repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move plugin install CRUD**

- Move:
  - `upsert_openclaw_plugin_install_with_pool`
  - `list_openclaw_plugin_installs_with_pool`
  - `delete_openclaw_plugin_install_with_pool`
  - `get_openclaw_plugin_install_by_id_with_pool`
- Keep normalization helpers near the repo only if they are purely persistence-oriented; otherwise leave them in the root until Task 3.

**Step 2: Keep the public surface stable**

- Re-export these functions from the root file.
- Do not change table names, sort order, or install/update timestamp behavior.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib upsert_openclaw_plugin_install_records_plugin_metadata -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib list_openclaw_plugin_installs_is_separate_from_local_skills -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib upsert_openclaw_plugin_install_updates_existing_record -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/install_repo.rs
git commit -m "refactor(runtime): extract openclaw plugin install repo"
```

## Task 3: Extract installation flow into `install_service.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/install_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move filesystem and npm install flow**

- Move:
  - `install_openclaw_plugin_from_npm`
  - workspace preparation helpers
  - `resolve_installed_package_dir`
  - package manifest loading helpers
  - plugin manifest loading helpers that belong to install resolution

**Step 2: Preserve user-visible behavior**

- Keep the same localized install failure message text.
- Keep the same workspace folder layout and package.json bootstrap format.
- Continue persisting through the repo layer extracted in Task 2.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib upsert_openclaw_plugin_install_records_plugin_metadata -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib upsert_openclaw_plugin_install_updates_existing_record -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/install_service.rs
git commit -m "refactor(runtime): extract openclaw plugin install service"
```

## Task 4: Extract plugin-host inspect and snapshot flow into `plugin_host_service.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/plugin_host_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move one-shot plugin-host execution helpers**

- Move:
  - plugin-host path resolution helpers
  - inspect script execution
  - channel snapshot execution
  - `inspect_openclaw_plugin_with_pool`
  - `get_openclaw_plugin_feishu_channel_snapshot_with_pool`
  - `list_openclaw_plugin_channel_hosts_with_pool`
  - `derive_channel_capabilities`
  - inspection-to-channel-host mapping helpers

**Step 2: Keep this slice separate from runtime supervision**

- Do not move long-lived process state into this module.
- Keep this module limited to request/response style child-process execution.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib resolve_plugin_host_dir_finds_packaged_up_directory -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib build_plugin_host_fixture_root_uses_app_data_dir -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib derive_channel_capabilities_flattens_runtime_flags -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/plugin_host_service.rs
git commit -m "refactor(runtime): extract openclaw plugin host service"
```

## Task 5: Extract advanced settings and config projection into `settings_service.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/settings_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move advanced settings projection**

- Move:
  - `get_openclaw_plugin_feishu_advanced_settings_with_pool`
  - `set_openclaw_plugin_feishu_advanced_settings_with_pool`
  - setting defaulting helpers
  - Feishu config projection helpers such as `build_feishu_openclaw_config_*`

**Step 2: Keep app-setting semantics stable**

- Preserve current default values for render mode, chunking, markdown behavior, heartbeat, and dynamic agent creation settings.
- Keep the exact `app_settings` keys unchanged.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib feishu_advanced_settings_round_trip_through_app_settings -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib feishu_advanced_settings_returns_projection_defaults_when_unset -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib feishu_advanced_settings_treats_blank_rows_as_unset_defaults -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib build_feishu_openclaw_config_projects_official_defaults -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib build_feishu_openclaw_config_projects_employee_accounts_with_inherited_defaults -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/settings_service.rs
git commit -m "refactor(runtime): extract openclaw plugin settings service"
```

## Task 6: Extract setup progress and credential probe flow into `setup_service.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/setup_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move setup/status logic**

- Move:
  - environment probe helpers
  - setup summary derivation
  - `get_feishu_setup_progress_with_pool`
  - auto-restore decision helpers
  - shim and controlled-state credential sync helpers
  - Feishu credential probe helpers and response parsers

**Step 2: Preserve startup-facing behavior**

- Keep `maybe_restore_openclaw_plugin_feishu_runtime_with_pool` at the root until Task 7, but make it call the extracted setup helpers.
- Keep the startup decision inputs identical so `lib.rs` restore behavior remains unchanged.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib derives_environment_status_when_node_and_npm_are_available -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib derives_setup_summary_state_for_fully_ready_flow -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib auto_restore_feishu_runtime_when_previous_connection_was_fully_approved -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib sync_feishu_gateway_credentials_from_shim_updates_app_settings -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib sync_feishu_gateway_credentials_from_controlled_state_reads_env_secret -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib parse_feishu_app_access_token_response_returns_token_on_success -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib parse_feishu_bot_info_response_extracts_identity -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/setup_service.rs
git commit -m "refactor(runtime): extract openclaw plugin setup service"
```

## Task 7: Extract Feishu runtime state and event bridge into `runtime_state.rs` and `runtime_bridge.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_state.rs`
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_bridge.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/tests/test_feishu_gateway.rs`

**Step 1: Move runtime state and status shaping**

- Move into `runtime_state.rs`:
  - runtime state structs
  - log trimming
  - status merge helpers
  - runtime status getters
  - process-command-line matching helpers if they stay tied to runtime supervision

**Step 2: Move runtime bridge and dispatch flow**

- Move into `runtime_bridge.rs`:
  - outbound send payload building
  - pending waiter registration and cleanup
  - `send_result` and `command_error` event parsing and delivery
  - runtime dispatch event parsing into `ImEvent`
  - pairing request event handling
  - thread-id resolution helpers
  - runtime start/stop helpers and stale-process cleanup if the state/bridge split stays readable

**Step 3: Keep exported runtime symbols stable**

- Re-export:
  - `OpenClawPluginFeishuRuntimeState`
  - `OpenClawPluginFeishuRuntimeStatus`
  - outbound request/result DTOs
  - runtime outbound helper functions used by `feishu_gateway.rs`

**Step 4: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib outbound_send_writes_command_and_receives_structured_send_result -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib outbound_command_error_fails_pending_request_immediately -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib merges_runtime_status_patch_events -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib merges_runtime_fatal_events_into_last_error -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib parses_runtime_dispatch_events_into_im_events -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib resolves_runtime_dispatch_thread_id_from_pairing_chat_id -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway -- --nocapture`

Expected:
- PASS

**Step 5: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_state.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/runtime_bridge.rs apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): extract openclaw plugin runtime bridge"
```

## Task 8: Extract installer control flow into `installer_session.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/installer_session.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`

**Step 1: Move installer-session control**

- Move:
  - `start_openclaw_lark_installer_session_with_pool`
  - `stop_openclaw_lark_installer_session_in_state`
  - `send_openclaw_lark_installer_input_in_state`
  - shim script generation helpers
  - shim path helpers
  - installer auto-input and prompt-hint helpers
  - installer status helpers

**Step 2: Keep process behavior stable**

- Preserve current auto-input defaults for `create` vs `link`.
- Preserve current prompt-hint wording.
- Preserve current recent-output truncation semantics.

**Step 3: Verify**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib installer_auto_input_selects_create_mode_by_default -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib installer_auto_input_selects_link_mode_and_sends_credentials -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib installer_prompt_hint_explains_poll_waiting_states -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openclaw_shim_script_supports_minimal_installer_commands -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib ensure_openclaw_cli_shim_creates_files -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openclaw_lark_tools_args_follow_official_wrapper_shape -- --nocapture`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins/installer_session.rs
git commit -m "refactor(runtime): extract openclaw lark installer session"
```

## Task 9: Move Tauri wrappers and tests into `tauri_commands.rs` and `tests.rs`

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/tauri_commands.rs`
- Create: `apps/runtime/src-tauri/src/commands/openclaw_plugins/tests.rs`
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Test: `apps/runtime/src-tauri/tests/test_feishu_gateway.rs`

**Step 1: Move implementation bodies behind thin wrappers**

- Move the non-trivial command implementation bodies into `tauri_commands.rs`.
- Keep macro-visible root wrappers in `openclaw_plugins.rs` if Tauri macro visibility still requires it.
- The root file should end as shell plus `pub use` compatibility layer.

**Step 2: Move file-local tests out of the root**

- Move the root `#[cfg(test)]` module into `openclaw_plugins/tests.rs`.
- Keep any helper setup functions local to the new tests module.

**Step 3: Final verification**

Run:
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openclaw_plugins -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway -- --nocapture`
- `pnpm test:rust-fast`

Expected:
- PASS

**Step 4: Checkpoint**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_plugins.rs apps/runtime/src-tauri/src/commands/openclaw_plugins apps/runtime/src-tauri/tests/test_feishu_gateway.rs
git commit -m "refactor(runtime): finish openclaw plugins command split"
```

## Done When

- `openclaw_plugins.rs` is a thin compatibility shell with macro-visible wrappers and stable re-exports.
- implementation logic lives in focused child modules under `apps/runtime/src-tauri/src/commands/openclaw_plugins/`.
- `lib.rs` startup restore path still works through stable imports.
- `feishu_gateway.rs` and `test_feishu_gateway.rs` continue to compile against the root export surface.
- no replacement child module becomes another `800+` line dumping ground during the split.

## Verification Checklist

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib openclaw_plugins -- --nocapture`
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_feishu_gateway -- --nocapture`
- `pnpm test:rust-fast`

Plan complete and saved to `docs/plans/2026-03-23-rust-openclaw-plugins-split-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration.
2. Parallel Session (separate) - Open a new session with `executing-plans`, batch execution with checkpoints.
