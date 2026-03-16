# OpenClaw Browser Skill Compatibility P0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the minimum WorkClaw compatibility layer needed to run `xiaohongshu-ops-skill` through the half-publish flow with a durable `openclaw` browser profile.

**Architecture:** Keep the existing WorkClaw browser stack intact, then add a session-scoped OpenClaw compatibility layer on top of it. The Rust runtime will expose OpenClaw-style tool names, while the sidecar will gain a single compatibility endpoint backed by a persistent `openclaw` profile and real tab/`targetId` management.

**Tech Stack:** Rust, Tauri, reqwest blocking tool bridge, TypeScript, Hono, Playwright, Rust integration tests, Node test runner

---

### Task 1: Add the unified `browser` compatibility tool in Rust

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/browser_compat.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_tool_setup.rs`
- Test: `apps/runtime/src-tauri/tests/test_browser_compat.rs`

**Step 1: Write the failing test**

```rust
use runtime_lib::agent::ToolRegistry;
use runtime_lib::agent::tools::browser_compat::register_browser_compat_tool;

#[test]
fn test_register_browser_compat_tool() {
    let registry = ToolRegistry::new();
    register_browser_compat_tool(&registry, "http://localhost:8765");

    let tool = registry.get("browser").expect("browser should be registered");
    let schema = tool.input_schema();

    assert!(schema["properties"]["action"].is_object());
    assert!(schema["properties"]["profile"].is_object());
    assert!(schema["properties"]["targetId"].is_object());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_compat -- --nocapture`

Expected: FAIL because `browser_compat` does not exist yet.

**Step 3: Write minimal implementation**

```rust
pub fn register_browser_compat_tool(registry: &ToolRegistry, sidecar_url: &str) {
    registry.register(Arc::new(SidecarBridgeTool::new(
        sidecar_url.to_string(),
        "/api/browser/compat".to_string(),
        "browser".to_string(),
        "OpenClaw-compatible browser tool for profile-aware browser actions.".to_string(),
        browser_compat_schema(),
    )));
}
```

Then call `register_browser_compat_tool(...)` from `prepare_runtime_tools(...)` after the existing `register_browser_tools(...)` call.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_compat -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/browser_compat.rs apps/runtime/src-tauri/src/agent/tools/mod.rs apps/runtime/src-tauri/src/commands/chat_tool_setup.rs apps/runtime/src-tauri/tests/test_browser_compat.rs
git commit -m "feat: add openclaw-compatible browser tool"
```

### Task 2: Add session-scoped `read` / `find` / `ls` / `exec` aliases

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/tool_alias.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_tool_setup.rs`
- Test: `apps/runtime/src-tauri/tests/test_tool_aliases.rs`

**Step 1: Write the failing test**

```rust
use runtime_lib::agent::ToolRegistry;
use runtime_lib::agent::tools::{register_tool_alias, ReadFileTool, GlobTool, ListDirTool, BashTool};
use std::sync::Arc;

#[test]
fn test_openclaw_style_aliases_delegate_to_existing_tools() {
    let registry = ToolRegistry::new();
    let read = Arc::new(ReadFileTool);
    let find = Arc::new(GlobTool);
    let ls = Arc::new(ListDirTool);
    let exec = Arc::new(BashTool::new());

    registry.register(read.clone());
    registry.register(find.clone());
    registry.register(ls.clone());
    registry.register(exec.clone());

    register_tool_alias(&registry, "read", read);
    register_tool_alias(&registry, "find", find);
    register_tool_alias(&registry, "ls", ls);
    register_tool_alias(&registry, "exec", exec);

    assert!(registry.get("read").is_some());
    assert!(registry.get("find").is_some());
    assert!(registry.get("ls").is_some());
    assert!(registry.get("exec").is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_tool_aliases -- --nocapture`

Expected: FAIL because the alias helper does not exist yet.

**Step 3: Write minimal implementation**

```rust
pub struct ToolAlias {
    alias: String,
    inner: Arc<dyn Tool>,
}

impl Tool for ToolAlias {
    fn name(&self) -> &str { &self.alias }
    fn description(&self) -> &str { self.inner.description() }
    fn input_schema(&self) -> Value { self.inner.input_schema() }
    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        self.inner.execute(input, ctx)
    }
}
```

Register these aliases only inside `prepare_runtime_tools(...)`, not in `ToolRegistry::with_standard_tools()`.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_tool_aliases -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/tool_alias.rs apps/runtime/src-tauri/src/agent/tools/mod.rs apps/runtime/src-tauri/src/commands/chat_tool_setup.rs apps/runtime/src-tauri/tests/test_tool_aliases.rs
git commit -m "feat: add openclaw-style tool aliases"
```

### Task 3: Add a durable `openclaw` profile lifecycle to the sidecar

**Files:**
- Modify: `apps/runtime/sidecar/src/browser.ts`
- Modify: `apps/runtime/sidecar/src/index.ts`
- Test: `apps/runtime/sidecar/test/browser.local-automation.test.ts`
- Test: `apps/runtime/sidecar/test/browser.compat-api.test.ts`

**Step 1: Write the failing tests**

```ts
test('start profile creates durable openclaw profile state', async () => {
  const controller = new BrowserController();
  const result = await controller.compat({ action: 'start', profile: 'openclaw' });
  assert.equal(result.profile, 'openclaw');
  assert.equal(result.running, true);
});

test('profiles lists only the supported openclaw profile in p0', async () => {
  const controller = new BrowserController();
  await controller.compat({ action: 'start', profile: 'openclaw' });
  const result = await controller.compat({ action: 'profiles' });
  assert.match(JSON.stringify(result), /openclaw/);
});
```

**Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.(local-automation|compat-api)"`

Expected: FAIL because profile-aware compatibility actions do not exist yet.

**Step 3: Write minimal implementation**

Implement a compatibility action dispatcher in `BrowserController` and persist a single supported profile:

```ts
type CompatAction = 'status' | 'start' | 'profiles' | 'tabs' | 'open' | 'focus' | 'snapshot' | 'act' | 'upload';

type ProfileState = {
  profile: 'openclaw';
  context: BrowserContext;
  pages: Map<string, Page>;
  activeTargetId: string | null;
};
```

Store the durable profile data under an app-owned directory and reject unsupported profiles with a direct error.

**Step 4: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.(local-automation|compat-api)"`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/browser.ts apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/browser.local-automation.test.ts apps/runtime/sidecar/test/browser.compat-api.test.ts
git commit -m "feat: add durable openclaw browser profile lifecycle"
```

### Task 4: Add `tabs`, `open`, `focus`, and stable `targetId` handling

**Files:**
- Modify: `apps/runtime/sidecar/src/browser.ts`
- Modify: `apps/runtime/sidecar/src/index.ts`
- Test: `apps/runtime/sidecar/test/browser.compat-api.test.ts`

**Step 1: Write the failing tests**

```ts
test('open returns a stable targetId and tabs exposes it', async () => {
  const controller = buildCompatController();
  const opened = await controller.compat({ action: 'open', profile: 'openclaw', url: 'https://example.com' });
  const tabs = await controller.compat({ action: 'tabs', profile: 'openclaw' });

  assert.equal(typeof opened.targetId, 'string');
  assert.match(JSON.stringify(tabs), new RegExp(opened.targetId));
});

test('snapshot and act use the requested targetId', async () => {
  const controller = buildCompatController();
  const { targetId } = await controller.compat({ action: 'open', profile: 'openclaw', url: 'https://example.com' });
  const snap = await controller.compat({ action: 'snapshot', profile: 'openclaw', targetId });
  assert.equal(snap.targetId, targetId);
});
```

**Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.compat-api"`

Expected: FAIL because `targetId` is not yet backed by real profile-local tab state.

**Step 3: Write minimal implementation**

Create a profile-local page registry:

```ts
function targetIdForPage(page: Page): string {
  return `tab_${page.guid ?? crypto.randomUUID()}`;
}
```

Then ensure:

- `open` creates or tracks a page and returns `targetId`
- `tabs` returns `{ targetId, url, title, active }[]`
- `focus` updates `activeTargetId`
- `snapshot` and `act` resolve `targetId` back to the stored page

**Step 4: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.compat-api"`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/browser.ts apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/browser.compat-api.test.ts
git commit -m "feat: add tab and targetId handling for browser compatibility"
```

### Task 5: Add `browser.upload` and `/tmp/openclaw/uploads` path staging

**Files:**
- Create: `apps/runtime/sidecar/src/browser_uploads.ts`
- Modify: `apps/runtime/sidecar/src/browser.ts`
- Modify: `apps/runtime/sidecar/src/index.ts`
- Test: `apps/runtime/sidecar/test/browser.compat-api.test.ts`

**Step 1: Write the failing tests**

```ts
test('upload accepts a normal local file path and stages it', async () => {
  const controller = buildCompatController();
  const result = await controller.compat({
    action: 'upload',
    profile: 'openclaw',
    targetId: 'tab_1',
    inputRef: 'e3',
    paths: ['E:/tmp/cover.png'],
  });
  assert.equal(result.ok, true);
});

test('upload maps /tmp/openclaw/uploads to a workclaw-owned staging path', async () => {
  const mapped = mapOpenClawUploadPath('/tmp/openclaw/uploads/cover.png');
  assert.match(mapped, /openclaw[\\\\/]uploads/i);
});
```

**Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.compat-api"`

Expected: FAIL because upload compatibility and path staging do not exist yet.

**Step 3: Write minimal implementation**

```ts
export function mapCompatUploadPath(inputPath: string, stagingRoot: string): string {
  if (inputPath.startsWith('/tmp/openclaw/uploads/')) {
    return path.join(stagingRoot, path.basename(inputPath));
  }
  return inputPath;
}
```

Then implement `upload` so it stages incoming files and calls Playwright file upload on the resolved page.

**Step 4: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.compat-api"`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/browser_uploads.ts apps/runtime/sidecar/src/browser.ts apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/browser.compat-api.test.ts
git commit -m "feat: add upload compatibility for openclaw browser skills"
```

### Task 6: Wire prompt exposure, diagnostics, and regression coverage

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat_tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/sidecar.rs`
- Modify: `apps/runtime/src-tauri/tests/test_tools_complete.rs`
- Modify: `apps/runtime/src-tauri/tests/test_browser_tools.rs`
- Modify: `docs/browser-automation-integration.md`

**Step 1: Write the failing tests**

```rust
#[test]
fn test_runtime_registration_exposes_browser_and_openclaw_aliases() {
    let registry = ToolRegistry::with_standard_tools();
    assert!(registry.get("browser").is_none(), "compat tools should stay session-scoped");
}
```

Add a runtime-level test that prepares a chat session registry and asserts the prepared registry exposes:

- `browser`
- `read`
- `find`
- `ls`
- `exec`

Also add a sidecar-start failure assertion that confirms the real failure reason is preserved in logs or returned diagnostics.

**Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_tools_complete -- --nocapture`

Expected: FAIL because the compatibility tools and diagnostics assertions are not wired yet.

**Step 3: Write minimal implementation**

- register compat tools in `prepare_runtime_tools(...)`
- emit sidecar/browser compatibility log lines with action/profile/targetId
- update docs to describe the new compatibility layer and P0 limits

**Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_browser_compat -- --nocapture`

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_tool_aliases -- --nocapture`

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_tools_complete -- --nocapture`

Run: `pnpm --dir apps/runtime/sidecar test -- --test-name-pattern="browser.(local-automation|compat-api)"`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_tool_setup.rs apps/runtime/src-tauri/src/sidecar.rs apps/runtime/src-tauri/tests/test_tools_complete.rs apps/runtime/src-tauri/tests/test_browser_tools.rs docs/browser-automation-integration.md
git commit -m "feat: wire openclaw browser compatibility into runtime"
```

### Task 7: Run the P0 manual acceptance flow for `xiaohongshu-ops-skill`

**Files:**
- Modify: `docs/browser-automation-integration.md`
- Modify: `docs/plans/2026-03-16-openclaw-browser-skill-compatibility-design.md`

**Step 1: Prepare the runtime**

Run: `pnpm app`

Expected: WorkClaw desktop launches and the sidecar starts without a browser timeout.

**Step 2: Import the target skill and create a clean test session**

Use the app to import:

- `https://github.com/Xiangyu-CAS/xiaohongshu-ops-skill`

Expected: the skill appears in the visible skill list for the session and is projected into the session workspace.

**Step 3: Validate first-login persistence**

- Start a session with the skill
- Let the runtime open the Xiaohongshu creator flow with `profile="openclaw"`
- Complete the manual QR login once if needed
- Restart the app and re-run the flow

Expected: second run reuses the existing login state.

**Step 4: Validate half-publish behavior**

Drive one real flow until:

- upload area is filled
- title is filled
- body is filled
- publish button is visible
- final publish click is not performed

Expected: the session stops at the publish page and clearly reports that it is waiting before the final click.

**Step 5: Commit**

```bash
git add docs/browser-automation-integration.md docs/plans/2026-03-16-openclaw-browser-skill-compatibility-design.md
git commit -m "docs: record openclaw browser compatibility acceptance flow"
```
