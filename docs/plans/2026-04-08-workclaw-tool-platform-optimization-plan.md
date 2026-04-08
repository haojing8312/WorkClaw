# WorkClaw Tool Platform Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade WorkClaw's tool layer from a thin runtime registry into a consistent tool platform with explicit metadata, centralized registration, and a unified permission decision contract.

**Architecture:** Keep the existing Rust execution loop and concrete tools intact in phase 1, then add a typed metadata layer and registry builder around them. After that, route approval and policy checks through a shared decision object so file, shell, browser, MCP, and plugin-backed tools all report capability and risk the same way.

**Tech Stack:** Rust, Tauri, serde/serde_json, existing WorkClaw agent runtime, runtime-policy crate, Vitest/Playwright-facing desktop UI contracts, Rust unit and integration tests.

---

### Task 1: Snapshot The Current Tool Surface

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_support.rs`
- Test: `apps/runtime/src-tauri/src/agent/registry.rs`

**Step 1: Write the failing test**

Add a Rust unit test in `apps/runtime/src-tauri/src/agent/registry.rs` that builds `ToolRegistry::with_standard_tools()` and asserts the resulting tool definitions include stable names for the current baseline tools such as `read_file`, `write_file`, `glob`, `grep`, `edit`, `list_dir`, `file_stat`, `file_delete`, `file_move`, `file_copy`, `todo_write`, `web_fetch`, `bash`, `screenshot`, and `open_in_folder`.

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test agent_registry`

Expected: FAIL because there is no stable snapshot-style assertion yet.

**Step 3: Write minimal implementation**

Add a helper on the registry side that returns a stable, sorted list of tool names for tests and prompt assembly. Keep existing runtime behavior unchanged.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test agent_registry`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/registry.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_support.rs
git commit -m "test(tooling): snapshot standard tool surface"
```

## Follow-on Status

On 2026-04-08, the follow-on deferred-loading phase was completed in a separate design and plan:

- `docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-design.md`
- `docs/plans/2026-04-08-workclaw-tool-defer-loading-and-selection-plan.md`

That follow-on work added:

- staged tool exposure with `full/recommended/active/deferred`
- conservative expansion from deferred exposure to full exposure
- structured tool discovery candidates with match reasons
- route-side recommendation summaries in runtime records

### Task 2: Add Tool Metadata Types

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Create: `apps/runtime/src-tauri/src/agent/tool_manifest.rs`
- Test: `apps/runtime/src-tauri/src/agent/tool_manifest.rs`

**Step 1: Write the failing test**

Add unit tests for a new `ToolMetadata` / `ToolManifestEntry` type covering:
- default display name fallback to tool name
- category serialization
- read-only vs destructive flags
- approval hint presence

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test tool_manifest`

Expected: FAIL because the metadata types do not exist.

**Step 3: Write minimal implementation**

Introduce a small metadata model with fields:
- `name`
- `display_name`
- `category`
- `read_only`
- `destructive`
- `concurrency_safe`
- `open_world`
- `requires_approval`
- `source`

Expose a default `metadata()` method on the `Tool` trait that tools can override incrementally without breaking existing implementations.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test tool_manifest`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/types.rs apps/runtime/src-tauri/src/agent/tool_manifest.rs
git commit -m "feat(tooling): add tool metadata model"
```

### Task 3: Teach Core Tools To Report Metadata

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/read_file.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/write_file.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/edit_tool.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/bash.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/web_search.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/web_fetch.rs`
- Test: `apps/runtime/src-tauri/src/agent/registry.rs`

**Step 1: Write the failing test**

Extend registry tests to assert a few representative tools publish the expected metadata:
- `read_file` is `read_only=true`
- `write_file` is `destructive=true`
- `bash` is `requires_approval=true` or otherwise flagged as risky
- `web_search` is `open_world=true`

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test agent_registry`

Expected: FAIL because tool metadata still falls back to defaults.

**Step 3: Write minimal implementation**

Override `metadata()` on the representative core tools first. Do not try to annotate every tool in one pass; cover the categories that drive the first UI and permission decisions.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test agent_registry`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/read_file.rs apps/runtime/src-tauri/src/agent/tools/write_file.rs apps/runtime/src-tauri/src/agent/tools/edit_tool.rs apps/runtime/src-tauri/src/agent/tools/bash.rs apps/runtime/src-tauri/src/agent/tools/web_search.rs apps/runtime/src-tauri/src/agent/tools/web_fetch.rs apps/runtime/src-tauri/src/agent/registry.rs
git commit -m "feat(tooling): annotate core tool metadata"
```

### Task 4: Centralize Runtime Tool Registration

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/tool_registry_builder.rs`
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`

**Step 1: Write the failing test**

Add a test that constructs the runtime tool setup path and asserts:
- standard tools are present
- runtime-only tools such as `exec`, `task`, `skill`, `compact`, `ask_user` are present
- aliases like `read`, `find`, `ls` remain resolvable

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test tool_setup`

Expected: FAIL because there is no single builder output to inspect.

**Step 3: Write minimal implementation**

Extract registration into a builder that stages:
- standard tools
- process-managed shell tools
- browser tools
- runtime collaboration tools
- search tool or MCP fallback
- memory and skill tools
- aliases

Keep `prepare_runtime_tools()` as the orchestration entrypoint, but make it call the new builder so the session-visible tool surface is assembled in one place.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test tool_setup`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_registry_builder.rs apps/runtime/src-tauri/src/agent/registry.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs
git commit -m "refactor(tooling): centralize runtime tool registration"
```

### Task 5: Emit A Session Tool Manifest

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/registry.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_support.rs`
- Modify: `apps/runtime/src/lib/chat-stream-events.ts`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

**Step 1: Write the failing test**

Add a frontend-facing test that expects the session state to expose a manifest-like payload with tool names and metadata flags, without changing existing message rendering.

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- ChatView.side-panel-redesign.test.tsx`

Expected: FAIL because no tool manifest payload exists yet.

**Step 3: Write minimal implementation**

Expose a serialized manifest from the Rust runtime support layer and thread it into the existing frontend types. Do not redesign the UI yet; only make the metadata available for later use.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- ChatView.side-panel-redesign.test.tsx`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/registry.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/runtime_support.rs apps/runtime/src/lib/chat-stream-events.ts apps/runtime/src/types.ts apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "feat(tooling): expose session tool manifest"
```

### Task 6: Introduce A Unified Permission Decision Type

**Files:**
- Create: `packages/runtime-policy/src/tool_decision.rs`
- Modify: `packages/runtime-policy/src/lib.rs`
- Modify: `packages/runtime-policy/src/permissions.rs`
- Test: `packages/runtime-policy/tests/permissions.rs`

**Step 1: Write the failing test**

Add policy tests for a unified decision result that can represent:
- `allow`
- `ask`
- `deny`
- optional `reason`
- optional `fingerprint`

Cover representative cases:
- `read_file` => allow
- destructive `bash` => ask
- `file_delete` => ask

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test permissions`

Expected: FAIL because decision objects do not exist yet.

**Step 3: Write minimal implementation**

Add a typed decision layer in `runtime-policy` and implement an adapter from the current risk classifier into this new decision object. Keep old helper APIs available temporarily to avoid a big-bang refactor.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test permissions`

Expected: PASS

**Step 5: Commit**

```bash
git add packages/runtime-policy/src/tool_decision.rs packages/runtime-policy/src/lib.rs packages/runtime-policy/src/permissions.rs packages/runtime-policy/tests/permissions.rs
git commit -m "feat(policy): add unified tool permission decision"
```

### Task 7: Route Tool Dispatch Through The New Permission Decision

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/permissions.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`

**Step 1: Write the failing test**

Add tool dispatch tests covering:
- allow path executes directly
- ask path emits approval flow
- deny path returns a policy-blocked error without executing the tool

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test tool_dispatch`

Expected: FAIL because dispatch still only understands the old confirmation check.

**Step 3: Write minimal implementation**

Update dispatch to consume the new policy decision object before execution. Preserve current approval bus behavior, but base it on explicit `ask` rather than re-deriving risk in multiple places.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test tool_dispatch`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs apps/runtime/src-tauri/src/agent/permissions.rs
git commit -m "refactor(policy): route tool dispatch through unified decisions"
```

### Task 8: Add A Typed Tool Result Envelope

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/tool_result.rs`
- Modify: `apps/runtime/src-tauri/src/agent/types.rs`
- Modify: representative tools in `apps/runtime/src-tauri/src/agent/tools/`
- Test: `packages/runtime-executor-core/tests/output.rs`

**Step 1: Write the failing test**

Add output tests for a stable envelope structure with:
- `tool`
- `summary`
- `data`
- `error`
- `artifacts`

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test output`

Expected: FAIL because tool outputs are still mixed text and JSON-string payloads.

**Step 3: Write minimal implementation**

Introduce a typed helper in `tool_result.rs` and migrate a small set of tools first, starting with file and web tools that already produce structured content. Preserve textual summaries so the existing transcript UI does not regress.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test output`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/tool_result.rs apps/runtime/src-tauri/src/agent/types.rs packages/runtime-executor-core/tests/output.rs
git commit -m "feat(tooling): add typed tool result envelope"
```

### Task 9: Define Tool Profiles For Runtime Assembly

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/tool_profiles.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_registry_builder.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs`

**Step 1: Write the failing test**

Add tests for profile-based assembly such as:
- `safe_default`
- `coding`
- `browser`
- `employee`

Assert each profile resolves into a predictable allowed tool set.

**Step 2: Run test to verify it fails**

Run: `pnpm test:rust-fast -- --test skill_routing_runner`

Expected: FAIL because tool assembly is still ad hoc.

**Step 3: Write minimal implementation**

Create profile definitions as named lists or filters over manifest metadata, then let skill/runtime assembly request a profile before layering explicit `allowed_tools`.

**Step 4: Run test to verify it passes**

Run: `pnpm test:rust-fast -- --test skill_routing_runner`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/tool_profiles.rs apps/runtime/src-tauri/src/agent/runtime/tool_registry_builder.rs apps/runtime/src-tauri/src/agent/runtime/skill_routing/runner.rs
git commit -m "feat(tooling): add named runtime tool profiles"
```

### Task 10: Run End-To-End Verification

**Files:**
- Modify: `docs/plans/2026-04-08-workclaw-tool-platform-optimization-plan.md`

**Step 1: Run Rust verification**

Run: `pnpm test:rust-fast`

Expected: PASS

**Step 2: Run sidecar/runtime verification relevant to changed surfaces**

Run: `pnpm test:sidecar`

Expected: PASS or clearly documented unrelated failures

**Step 3: Run desktop frontend verification for chat/tool UI**

Run: `pnpm --dir apps/runtime test -- ChatView.side-panel-redesign.test.tsx`

Expected: PASS

**Step 4: Record actual verification results**

Update this plan or the implementation journal with:
- commands run
- pass/fail status
- any known gaps

**Step 5: Commit**

```bash
git add docs/plans/2026-04-08-workclaw-tool-platform-optimization-plan.md
git commit -m "docs(plan): record tool platform verification status"
```

## Verification Record

### 2026-04-08 Tool Platform Pass

- `pnpm test:rust-fast`
  - PASS
  - Confirmed package-level coverage for:
    - `runtime-policy` permission decisions
    - `runtime-executor-core` tool output envelope handling
    - `runtime-models-app` and builtin skill support crates touched by shared runtime contracts
- `pnpm test:sidecar`
  - PASS
  - Result: `45` sidecar tests passed, `0` failed
- `pnpm --dir apps/runtime test -- ChatView.side-panel-redesign.test.tsx`
  - PASS
  - Result: `26` tests passed, `0` failed
- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
  - PASS
  - Used as the direct compile verification for the Rust runtime surfaces changed in this plan:
    - agent registry and tool manifest
    - runtime tool builder and tool setup
    - tool dispatch permission decisions
    - skill routing tool profiles

### Known Gaps

- I did not record a fresh full `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib` execution as a completion gate for this pass.
- Earlier in this session, direct runtime test-binary execution on this Windows environment hit `STATUS_ENTRYPOINT_NOT_FOUND`; the current pass therefore treats `--no-run` plus the broader `pnpm test:rust-fast` success as the reliable Rust-side verification evidence.
