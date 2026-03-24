# Rust OpenClaw Plugins Split Design

**Goal:** Turn [openclaw_plugins.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/openclaw_plugins.rs) into a maintainable Rust command surface by splitting it along real execution boundaries: plugin inventory, plugin-host inspection, Feishu runtime supervision, setup/settings orchestration, and the interactive Lark installer session. The split must preserve current Tauri command names, runtime state contracts, and the existing Feishu gateway call sites that already depend on this module.

## Why This Is The Hardest Rust Command File

[openclaw_plugins.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/openclaw_plugins.rs) is currently the largest command file in the Rust runtime at roughly 6.3k lines. It is harder to split than `employee_agents.rs`, `feishu_gateway.rs`, or `chat_*` because it combines several different execution models in one file:

- one-shot SQLite CRUD for installed plugin inventory
- one-shot Node/NPM process execution for install, inspect, and channel snapshot flows
- long-lived Feishu runtime process supervision with stdin/stdout event bridging
- computed setup/status pages that join settings, runtime state, installs, pairings, and routing data
- the interactive `openclaw-lark` installer session with auto-input and process lifecycle management
- Tauri command wrappers and test coverage for all of the above

That mixture makes the file a natural landing zone for new AI-generated work. Without a clearer boundary, feature work will keep accreting into the same root file because it already owns the types, helpers, and process state that other surfaces need.

## Current Boundary Map

### What The File Owns Today

- plugin installation persistence in `installed_openclaw_plugins`
- `npm install` based plugin installation
- plugin-host script resolution, execution, inspect, and snapshot collection
- channel host derivation and inspection summaries
- Feishu runtime state structs and runtime status helpers
- outbound runtime send request/response plumbing and pending waiter tracking
- Feishu gateway setup progress and advanced settings projection
- `openclaw-lark` installer session startup, input, auto-response, and shutdown
- Tauri command wrappers for all of the above
- file-local tests covering settings, runtime events, restore behavior, install CRUD, and command-line matching

### Who Depends On It

- [apps/runtime/src-tauri/src/lib.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/lib.rs) manages `OpenClawPluginFeishuRuntimeState` and `OpenClawLarkInstallerSessionState`, registers the command handlers, and calls `maybe_restore_openclaw_plugin_feishu_runtime_with_pool` during startup.
- [apps/runtime/src-tauri/src/commands/feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) imports multiple public helpers and state types from this module, including the Feishu runtime state and channel snapshot helpers.
- [apps/runtime/src-tauri/tests/test_feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/tests/test_feishu_gateway.rs) depends on the outbound runtime send hooks and runtime state types exported from this module.

This means the split must preserve public re-exports, or introduce compatibility shims in the root file, so that the existing call sites do not break while the internals are being decomposed.

## Recommended Design

### 1. Split By Execution Boundary, Not By Helper Count

The root mistake to avoid here is turning one giant file into many generic `service.rs` files. The responsibilities are more naturally separated by how they execute:

- database-backed plugin inventory
- one-shot plugin-host process execution
- long-running Feishu runtime supervision
- derived setup/settings view state
- interactive installer session control

That is the smallest set of boundaries that still matches the runtime behavior in the file today.

### 2. Keep The Root File As A Compatibility Shell

The root [openclaw_plugins.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/openclaw_plugins.rs) should ultimately keep only:

- thin `#[tauri::command]` wrappers
- public re-exports needed by `lib.rs`, `feishu_gateway.rs`, and tests
- a small amount of compatibility glue that Tauri macros or app-state wiring still require

It should stop owning the implementation details of install flows, plugin-host execution, runtime event handling, and installer process orchestration.

### 3. Preserve Existing Runtime Contracts

This split must not change:

- Tauri command names
- Tauri command return payloads
- installed plugin table semantics
- Feishu runtime state shape
- outbound request/result handshake behavior
- installer session state behavior
- startup restore behavior
- plugin-host inspect/snapshot output shape

That keeps the split internal and safe, even though the module is highly visible at runtime.

## Suggested Module Layout

The final structure under `apps/runtime/src-tauri/src/commands/openclaw_plugins/` should be organized by responsibility:

- `types.rs`
  - plugin install DTOs, inspection DTOs, runtime status DTOs, setup DTOs, outbound request/result DTOs, and any data shapes that are shared across child modules
- `install_repo.rs`
  - SQLite CRUD for `installed_openclaw_plugins`
  - row mapping, upsert/list/delete, and other persistence-only helpers
- `install_service.rs`
  - `npm install` workflow, workspace preparation, package manifest loading, installed package resolution, and install record creation
- `plugin_host_service.rs`
  - plugin-host script resolution, Node execution, inspect, channel snapshot, and channel host derivation
- `runtime_state.rs`
  - `OpenClawPluginFeishuRuntimeState`, runtime status structs, status merging, log trimming, and other state-only helpers
- `runtime_bridge.rs`
  - outbound request registration, send-result and command-error event handling, outbound message delivery, and pending waiter cleanup
- `settings_service.rs`
  - advanced settings get/set wrappers and app-setting projection logic
- `setup_service.rs`
  - Feishu setup progress computation, environment probing, auto-restore decision logic, and combined readiness status
- `installer_session.rs`
  - `openclaw-lark` installer session startup, prompt inference, auto-input, manual input, and shutdown
- `tauri_commands.rs`
  - the concrete command implementation bodies that the root file wraps
- `tests.rs`
  - module-level tests that are currently embedded in the giant root file

If the split needs one additional compatibility layer, keep that in the root file rather than inventing a new umbrella service.

## Responsibility Split

### Plugin Inventory

This slice owns the installed plugin catalog and the `npm install` workflow:

- validate and normalize install inputs
- prepare the workspace directory
- resolve the installed package directory
- read installed package metadata
- persist the install record
- list and delete installs

This slice is the safest first extraction because it is mostly deterministic and only depends on SQLite plus local filesystem operations.

### Plugin-Host Inspection

This slice owns all one-shot execution of the plugin-host scripts:

- resolve the plugin-host directory and script paths
- launch `inspect-plugin.mjs`
- launch the channel snapshot path
- convert inspection output into channel host records
- derive capability flags and display metadata

This is distinct from runtime supervision because it is short-lived process execution with request/response semantics rather than a persistent child process.

### Feishu Runtime Supervision

This slice owns the long-lived official Feishu runtime process:

- spawn and stop the runtime process
- manage stdin/stdout/stderr channels
- track runtime status, PID, port, errors, and recent logs
- merge runtime status events
- parse outbound `send_result` and `command_error` events
- manage pending outbound send waiters

This slice is the riskiest part of the file because it combines process lifecycle, shared mutable state, and event-driven cleanup.

### Setup And Settings

This slice owns the setup dashboard and the advanced settings projection:

- read/write Feishu advanced settings from `app_settings`
- derive setup progress from install state, runtime state, routing state, and pairing state
- probe local environment readiness
- decide whether the runtime should auto-restore after startup

This is a good example of logic that should stay separate from runtime supervision even though it reads some of the same state.

### Installer Session

This slice owns the interactive `openclaw-lark` installer flow:

- create the shim workspace
- write the shim wrapper script
- launch the official installer
- infer prompt hints from output
- auto-feed responses when possible
- accept manual input
- stop the installer session cleanly

This should be isolated from the Feishu runtime slice because its control flow is interactive and stateful in a different way.

### Tauri Wrapper Layer

This slice owns only macro-visible entrypoints:

- `start_openclaw_plugin_feishu_runtime`
- `stop_openclaw_plugin_feishu_runtime`
- `get_openclaw_plugin_feishu_runtime_status`
- `get_feishu_plugin_environment_status`
- `get_feishu_setup_progress`
- `get_openclaw_plugin_feishu_advanced_settings`
- `set_openclaw_plugin_feishu_advanced_settings`
- `start_openclaw_lark_installer_session`
- `get_openclaw_lark_installer_session_status`
- `send_openclaw_lark_installer_input`
- `stop_openclaw_lark_installer_session`
- `probe_openclaw_plugin_feishu_credentials`
- `upsert_openclaw_plugin_install`
- `list_openclaw_plugin_installs`
- `delete_openclaw_plugin_install`
- `inspect_openclaw_plugin`
- `list_openclaw_plugin_channel_hosts`
- `get_openclaw_plugin_feishu_channel_snapshot`
- `install_openclaw_plugin_from_npm`

The wrappers should remain thin and delegate immediately into the relevant child module.

## Recommended Smallest Safe Split Order

1. Extract `types.rs`.
2. Extract `install_repo.rs` and keep the root install functions as thin wrappers.
3. Extract `install_service.rs` for `npm install` and package manifest resolution.
4. Extract `plugin_host_service.rs` for inspect, snapshot, and channel-host derivation.
5. Extract `runtime_state.rs` and `runtime_bridge.rs` together so the outbound event plumbing stays coherent.
6. Extract `settings_service.rs` and `setup_service.rs` so progress logic is no longer mixed with runtime process code.
7. Extract `installer_session.rs`.
8. Move the macro-visible wrappers into `tauri_commands.rs`.
9. Move file-local tests into `tests.rs` as the final cleanup step.

The key sequencing rule is to keep the long-lived Feishu runtime and installer session logic for later, after the low-risk pure data and short-lived process slices have been isolated.

## Verification Suggestions

The verification should be staged to match the slice being extracted:

- after `types.rs` and `install_repo.rs`, run the install persistence tests that already exist in the file
- after `install_service.rs`, run the install record and install-update tests
- after `plugin_host_service.rs`, run the inspect / snapshot / host derivation tests
- after `runtime_state.rs` and `runtime_bridge.rs`, run the runtime send-result, command-error, and status-merge tests
- after `settings_service.rs` and `setup_service.rs`, run the advanced-settings round-trip and default-projection tests
- after `installer_session.rs`, run the installer prompt / status tests
- after the wrapper cleanup, run `pnpm test:rust-fast`

The important part is to keep each verification batch narrowly focused on the slice just extracted, so failures point to one boundary instead of the whole giant file.

## Risks

- breaking the `lib.rs` startup restore path if runtime state symbols move without re-exports
- breaking `feishu_gateway.rs` if channel snapshot or runtime helper names change
- creating a new giant child file that simply moves the mess instead of separating responsibilities
- separating the runtime bridge from the runtime state too aggressively and making the event flow harder to reason about
- mixing installer-session behavior back into the Feishu runtime slice because both interact with child processes

## Success Criteria

- [openclaw_plugins.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/openclaw_plugins.rs) becomes a thin compatibility shell instead of the implementation home for every plugin-related concern
- plugin inventory, plugin-host inspection, Feishu runtime supervision, setup/settings, and installer session logic each live in a focused child module
- the public Tauri command contract stays unchanged
- `lib.rs`, `feishu_gateway.rs`, and existing tests can continue to import the same public surface or a stable re-export layer
- the split pattern is clear enough that the next large command file can follow the same governance approach without inventing a new structure
