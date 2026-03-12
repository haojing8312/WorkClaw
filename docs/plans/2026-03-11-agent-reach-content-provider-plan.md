# Agent-Reach Content Provider Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a loosely coupled external content provider lane so WorkClaw can detect and use a user-installed Agent-Reach environment for reading and search tasks.

**Architecture:** Introduce a provider registry above existing content tools, then add a thin Agent-Reach adapter for detection, diagnostics, invocation, and result normalization. Keep browser interaction on the existing built-in browser lane and expose only provider-agnostic high-level content tools.

**Tech Stack:** Tauri commands in Rust, runtime React UI, existing tool registry, sidecar/runtime routing, Vitest, Cargo tests.

---

### Task 1: Define provider registry types and routing rules

**Files:**
- Create: `apps/runtime/src-tauri/src/content_providers/mod.rs`
- Create: `apps/runtime/src-tauri/src/content_providers/types.rs`
- Create: `apps/runtime/src-tauri/src/content_providers/router.rs`
- Test: `apps/runtime/src-tauri/tests/test_content_provider_router.rs`

**Step 1: Write the failing test**

Add route-selection tests for:

- `read_url` preferring `agent-reach`
- `search_content` preferring `agent-reach`
- browser interaction requests remaining unsupported by this router
- fallback to `builtin-web` when Agent-Reach is unavailable

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_router -- --nocapture`

Expected: FAIL because the provider registry and router do not exist yet.

**Step 3: Write minimal implementation**

Create a small provider model with:

- provider id
- availability state
- supported capabilities
- simple route selection inputs and outputs

Implement static routing rules only for the three phase-1 capabilities.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_router -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/content_providers apps/runtime/src-tauri/tests/test_content_provider_router.rs
git commit -m "feat(runtime): add content provider routing model"
```

### Task 2: Add Agent-Reach detection and diagnostics

**Files:**
- Create: `apps/runtime/src-tauri/src/content_providers/agent_reach.rs`
- Modify: `apps/runtime/src-tauri/src/content_providers/mod.rs`
- Test: `apps/runtime/src-tauri/tests/test_agent_reach_provider.rs`

**Step 1: Write the failing test**

Add tests covering:

- command missing -> `NotFound`
- command present but missing dependencies -> `Partial`
- healthy diagnostics -> `Available`

Use a fake command runner trait so the tests do not require Agent-Reach to be installed.

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_agent_reach_provider -- --nocapture`

Expected: FAIL because no adapter or diagnostics parser exists.

**Step 3: Write minimal implementation**

Implement:

- command existence probe
- a diagnostics runner abstraction
- a summary parser that maps command output into provider state

Keep parsing conservative and configuration-friendly.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_agent_reach_provider -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/content_providers/agent_reach.rs apps/runtime/src-tauri/src/content_providers/mod.rs apps/runtime/src-tauri/tests/test_agent_reach_provider.rs
git commit -m "feat(runtime): add agent-reach diagnostics adapter"
```

### Task 3: Add normalized content result contract

**Files:**
- Create: `apps/runtime/src-tauri/src/content_providers/result.rs`
- Modify: `apps/runtime/src-tauri/src/content_providers/types.rs`
- Test: `apps/runtime/src-tauri/tests/test_content_provider_result.rs`

**Step 1: Write the failing test**

Add tests for normalization into a common result structure with:

- source provider
- capability
- title
- url
- text
- markdown
- metadata

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_result -- --nocapture`

Expected: FAIL because the shared result type does not exist.

**Step 3: Write minimal implementation**

Define a serializable result contract and helper constructors for provider output normalization.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_result -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/content_providers/result.rs apps/runtime/src-tauri/src/content_providers/types.rs apps/runtime/src-tauri/tests/test_content_provider_result.rs
git commit -m "feat(runtime): add normalized content provider result"
```

### Task 4: Wire `read_url` through the provider registry

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/` relevant content-reading tool source
- Modify: `apps/runtime/src-tauri/src/content_providers/mod.rs`
- Test: `apps/runtime/src-tauri/tests/test_read_url_provider_routing.rs`

**Step 1: Write the failing test**

Add tests for:

- `read_url` using `agent-reach` when available
- fallback to built-in provider when unavailable
- normalized result shape reaching the tool response

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_read_url_provider_routing -- --nocapture`

Expected: FAIL because the tool is not yet registry-backed.

**Step 3: Write minimal implementation**

Refactor the existing read path so it resolves a provider through the registry before executing the request.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_read_url_provider_routing -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools apps/runtime/src-tauri/src/content_providers apps/runtime/src-tauri/tests/test_read_url_provider_routing.rs
git commit -m "feat(runtime): route read-url through content providers"
```

### Task 5: Wire `search_content` through the provider registry

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/` relevant search tool source
- Test: `apps/runtime/src-tauri/tests/test_search_content_provider_routing.rs`

**Step 1: Write the failing test**

Add tests for:

- cross-platform search preferring `agent-reach`
- fallback to current built-in search implementation
- result normalization for list responses

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_search_content_provider_routing -- --nocapture`

Expected: FAIL because search is not yet registry-backed.

**Step 3: Write minimal implementation**

Route search requests through the provider registry while preserving existing search fallback behavior.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_search_content_provider_routing -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools apps/runtime/src-tauri/tests/test_search_content_provider_routing.rs
git commit -m "feat(runtime): route content search through providers"
```

### Task 6: Wire `extract_media_context` through the provider registry

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/` relevant media extraction tool source
- Test: `apps/runtime/src-tauri/tests/test_extract_media_context_provider_routing.rs`

**Step 1: Write the failing test**

Add tests for:

- media URL extraction preferring `agent-reach`
- fallback behavior when provider is unavailable
- normalized metadata and transcript text shape

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_extract_media_context_provider_routing -- --nocapture`

Expected: FAIL because this tool path is not yet registry-backed.

**Step 3: Write minimal implementation**

Add provider-backed routing for media-context extraction and normalize the metadata contract.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_extract_media_context_provider_routing -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools apps/runtime/src-tauri/tests/test_extract_media_context_provider_routing.rs
git commit -m "feat(runtime): route media context extraction through providers"
```

### Task 7: Add Tauri commands for provider status and diagnostics

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Create: `apps/runtime/src-tauri/src/commands/content_providers.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_content_provider_commands.rs`

**Step 1: Write the failing test**

Add tests for commands that:

- list provider states
- run provider diagnostics
- return capability tags for the UI

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_commands -- --nocapture`

Expected: FAIL because the Tauri commands do not exist.

**Step 3: Write minimal implementation**

Add Tauri commands backed by the registry and Agent-Reach diagnostics adapter.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_commands -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/commands/content_providers.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_content_provider_commands.rs
git commit -m "feat(runtime): expose content provider diagnostics commands"
```

### Task 8: Add settings UI for external content providers

**Files:**
- Modify: `apps/runtime/src/` relevant settings or integrations view components
- Modify: `apps/runtime/src/` relevant API/invoke client
- Test: `apps/runtime/src/` relevant component test files

**Step 1: Write the failing test**

Add UI tests covering:

- Agent-Reach status badge
- capability tags
- diagnostics button behavior
- setup guide link rendering

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- --runInBand`

Expected: FAIL because the settings UI does not yet render provider data.

**Step 3: Write minimal implementation**

Add an `External Content Providers` section to the existing settings/integrations UI and wire it to the new Tauri commands.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime test -- --runInBand`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src
git commit -m "feat(ui): add external content provider diagnostics"
```

### Task 9: Document setup, routing, and fallback behavior

**Files:**
- Create: `docs/integrations/agent-reach-content-provider.md`
- Modify: `README.md`
- Modify: `README.en.md`

**Step 1: Write the docs changes**

Document:

- what Agent-Reach integration does and does not do
- required user-managed installation boundary
- diagnostics workflow
- supported phase-1 capabilities
- fallback behavior

**Step 2: Verify docs reference current product behavior**

Run: `rg -n "Agent-Reach|content provider|External Content Providers" docs README.md README.en.md`

Expected: New references point to the added setup guide and terminology is consistent.

**Step 3: Commit**

```bash
git add docs/integrations/agent-reach-content-provider.md README.md README.en.md
git commit -m "docs: add agent-reach content provider guide"
```

### Task 10: Run focused verification

**Files:**
- No source changes required

**Step 1: Run backend tests**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_content_provider_router --test test_agent_reach_provider --test test_content_provider_result --test test_read_url_provider_routing --test test_search_content_provider_routing --test test_extract_media_context_provider_routing --test test_content_provider_commands -- --nocapture`

Expected: PASS

**Step 2: Run frontend tests**

Run: `pnpm --dir apps/runtime test -- --runInBand`

Expected: PASS

**Step 3: Run targeted docs search**

Run: `rg -n "Agent-Reach|External Content Providers|read_url|search_content|extract_media_context" docs README.md README.en.md apps/runtime`

Expected: Key terms appear in code and docs with consistent naming.

**Step 4: Commit verification-ready branch state**

```bash
git status --short
git commit -m "chore: finalize agent-reach content provider integration"
```
