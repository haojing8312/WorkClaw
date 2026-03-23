# Rust Feishu Gateway Split Design

**Goal:** Turn [feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) into the next formal Rust command-splitting target after `employee_agents`, using the same large-file governance pattern while preserving the current Tauri and Feishu-facing contracts.

## Current Status

This split has now been implemented as the second formal Rust-side large-file governance template after `employee_agents`.

- Root [feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) is now `399` lines
- Runtime logic has been split into focused child modules under [feishu_gateway/](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway)
- The root file is now a thin shell plus a small amount of shared glue
- Internal module tests were also moved out of the root file into [tests.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway/tests.rs)

Implemented modules:

- `types.rs`
- `payload_parser.rs`
- `repo.rs`
- `gate_service.rs`
- `pairing_service.rs`
- `approval_service.rs`
- `outbound_service.rs`
- `relay_service.rs`
- `ingress_service.rs`
- `settings_service.rs`
- `planning_service.rs`
- `tauri_commands.rs`
- `tests.rs`

## Strategy Summary
- Change surface: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs` and its adjacent tests
- Affected modules: Tauri command exports, Feishu inbound parsing, gateway policy evaluation, pairing flow, approval notifications, outbound delivery, sidecar relay, employee-session bridge, related tests
- Main risk: breaking user-visible Feishu ingress, outbound delivery, or relay behavior while moving logic into child modules
- Recommended smallest safe path: keep the root file as the visible Tauri shell, then split the file into focused child modules by responsibility instead of one giant `service.rs`
- Required verification: focused cargo tests for parsing, gate decisions, pairing, and relay helpers; `pnpm test:rust-fast`; targeted Feishu gateway tests
- Release impact: low if contracts stay stable, but runtime-visible integration behavior means regressions would be user-facing

## Why This Is Next

[feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) originally exceeded `3200` lines and represented the clearest next giant command surface after `employee_agents`:

- it mixes external protocol handling with runtime orchestration
- it owns both inbound and outbound Feishu behavior
- it contains Tauri commands, relay state, persistence helpers, and gate rules in one file
- it already has a dedicated test file, which makes incremental extraction safer

This made it the best next candidate to apply the same split pattern that brought `employee_agents.rs` below the `800` threshold. That split is now complete enough that `feishu_gateway.rs` itself sits below the stricter `500` target line.

## Current Problem

The file currently mixes all of these concerns:

- Feishu webhook payload parsing and signature validation
- openclaw Feishu gate policy evaluation
- pairing request creation, approval, and resolution
- Feishu approval notification formatting and handling
- outbound send via official runtime and sidecar
- long connection / websocket status and reconciliation
- relay polling and inbound event bridge orchestration
- Tauri command entrypoints and app setting reads/writes

This creates three concrete maintenance problems:

1. a change to one Feishu surface drags unrelated runtime context into the edit
2. protocol parsing, policy, persistence, and command wrappers are hard to test independently
3. AI-native feature work is too likely to accrete into the root file because it is the easiest visible landing zone

## Recommended Design

### 1. Keep the root file as the visible command shell

The root [feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) should keep:

- public Tauri command wrappers
- public API surface that sibling modules already import
- shared type exports that truly belong to the domain boundary
- small compatibility glue required by Tauri macros or runtime state registration

It should stop owning full protocol parsing, gate rules, relay flow, or outbound orchestration.

### 2. Split by responsibility, not by helper count

Use focused child modules under `apps/runtime/src-tauri/src/commands/feishu_gateway/`:

- `types.rs`
  - gateway DTOs, payload structs, ws status structs, pairing records
- `payload_parser.rs`
  - payload parsing, mention extraction, text cleanup
- `repo.rs`
  - app setting reads/writes, credential lookup, sidecar URL resolution
- `gate_service.rs`
  - allowlist normalization, account/group gate config shaping, inbound gate decisions
- `pairing_service.rs`
  - pairing code generation, request creation, approval/deny flows, pairing account resolution
- `approval_service.rs`
  - approval-command parsing, approval notification text building, approval result handling
- `outbound_service.rs`
  - outbound route target shaping, official runtime send flow, sender/chat mapping helpers, runtime send hook handling
- `relay_service.rs`
  - sidecar JSON calls, chat listing, role summary push, ws event draining, long connection state, relay loop and ws role resolution
- `ingress_service.rs`
  - ingress auth/signature validation, default account resolution, employee connection lookup, inbound dispatch orchestration
- `settings_service.rs`
  - settings get/set wrappers
- `planning_service.rs`
  - role event and dispatch planning
- `tauri_commands.rs`
  - concrete command implementation bodies that the root file wraps

One important implementation note: the root file still keeps thin `#[tauri::command]` wrappers because Tauri's handler generation expects macro-visible command symbols in the root module. So the final shape is "thin shell + child implementation modules", not "pure re-export only".

### 3. Preserve the current behavior seams

The split should keep these seams stable:

- existing Tauri command names and payload shapes
- existing sidecar endpoint paths
- existing websocket/relay state shape
- existing `process_im_event` and `bridge_inbound_event_to_employee_sessions_with_pool` orchestration order
- existing approval and pairing semantics

This should be an internal structure refactor first, not a behavior redesign.

## Proposed Responsibility Split

### Command layer

- Tauri command wrappers only
- visible reconcile/start/stop/status command entrypoints
- preserve current argument shapes and return values

### Service layer

- inbound gate decisions
- pairing lifecycle orchestration
- approval-notification orchestration
- relay loop orchestration
- outbound delivery orchestration

### Repository/settings layer

- app setting reads and writes
- pairing request persistence
- any narrow Feishu-specific SQLite query helpers

### Adapter or gateway layer

- Feishu payload protocol parsing
- sidecar request execution
- official runtime outbound hook integration

## Suggested First-Cut Order

Completed extraction order:

1. `types.rs` and `payload_parser.rs`
2. `repo.rs`
3. `gate_service.rs`
4. `pairing_service.rs`
5. `approval_service.rs`
6. `outbound_service.rs`
7. `relay_service.rs`
8. `ingress_service.rs`
9. `settings_service.rs`
10. `planning_service.rs`
11. `tauri_commands.rs`
12. `tests.rs`

## What Not To Do

- do not replace one giant root file with one giant `service.rs`
- do not split purely by “small helper” without a real concern boundary
- do not move Tauri macros into child modules and rely only on re-export if it breaks macro visibility expectations
- do not couple sidecar transport code with gate policy logic in the same child file

## Recommended End State

The target shape now feels similar to `employee_agents`:

- root `feishu_gateway.rs` is a thin shell
- payload parsing is isolated
- gate policy is isolated
- pairing and approval flows are isolated
- outbound delivery is isolated
- relay runtime behavior is isolated
- tests no longer bloat the root file itself
- the root runtime file is below the `500` target line

## Success Criteria

- [feishu_gateway.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/commands/feishu_gateway.rs) is materially smaller
- no new giant replacement child file appears
- existing Tauri command contracts stay stable
- existing Feishu parsing and gateway tests stay green
- the split pattern remains recognizably aligned with the `employee_agents` reference template
