# Installer Sidecar Browser Runtime Fix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the packaged `WorkClaw_0.3.0_x64-setup.exe` runtime start its sidecar reliably and stop Xiaohongshu/OpenClaw-style skills from misdiagnosing WorkClaw as a missing external OpenClaw browser service.

**Architecture:** Keep the existing local sidecar model, but make `SidecarManager` resolve sidecar resources from both development and packaged layouts instead of assuming a source checkout with `node sidecar/dist/index.js`. In parallel, strengthen the runtime system prompt so imported OpenClaw-style skills are explicitly told to use WorkClaw's built-in `browser` compatibility tool and not require `openclaw-desktop.exe`, `D:\AI`, or a separately launched browser daemon.

**Tech Stack:** Rust (`tauri`, `reqwest`, integration tests), Tauri bundle config, TypeScript sidecar/Playwright, runtime prompt assembly tests.

---

### Task 1: Lock packaged sidecar startup expectations with failing Rust tests

**Files:**
- Modify: `apps/runtime/src-tauri/tests/test_sidecar.rs`
- Modify: `apps/runtime/src-tauri/src/sidecar.rs`

**Step 1: Write the failing tests**

Add focused tests for:
- dev layout resolution still finds `sidecar/dist/index.js`
- packaged layout resolution prefers bundled resources over `cwd`
- missing runtime returns a useful startup error instead of silently timing out

Example test shape:

```rust
#[test]
fn packaged_layout_prefers_bundled_sidecar_runtime() {
    let paths = SidecarRuntimePaths::for_tests(
        Some(PathBuf::from("C:/Program Files/WorkClaw/resources/sidecar/index.js")),
        Some(PathBuf::from("C:/Program Files/WorkClaw/resources/sidecar/node.exe")),
        PathBuf::from("C:/Program Files/WorkClaw"),
    );

    let resolved = resolve_sidecar_runtime(paths).expect("runtime should resolve");
    assert!(resolved.script.ends_with("resources/sidecar/index.js"));
    assert!(resolved.command.ends_with("node.exe"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_sidecar -- --nocapture`

Expected: FAIL because the current code has no packaged runtime resolution helpers and still assumes `node + cwd/sidecar/dist/index.js`.

**Step 3: Write minimal implementation**

In `apps/runtime/src-tauri/src/sidecar.rs`:
- extract runtime path resolution into testable helpers
- support development and packaged resource layouts
- surface a specific error when neither layout exists

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_sidecar -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/sidecar.rs apps/runtime/src-tauri/tests/test_sidecar.rs
git commit -m "fix: resolve packaged sidecar runtime layout"
```

---

### Task 2: Lock bundle config expectations with a failing installer config test

**Files:**
- Modify: `scripts/check-installer-branding.test.mjs`
- Modify: `apps/runtime/src-tauri/tauri.conf.json`

**Step 1: Write the failing test**

Extend the existing installer config test to assert that the Tauri bundle includes sidecar runtime resources.

Example assertions:

```js
assert.ok(Array.isArray(config?.bundle?.resources), "Expected bundle.resources to be configured");
assert.match(
  JSON.stringify(config.bundle.resources),
  /sidecar/i,
  "Expected bundle.resources to include sidecar runtime assets",
);
```

**Step 2: Run test to verify it fails**

Run: `node --test scripts/check-installer-branding.test.mjs`

Expected: FAIL because `tauri.conf.json` currently has no `bundle.resources`.

**Step 3: Write minimal implementation**

Update `apps/runtime/src-tauri/tauri.conf.json` so the Windows bundle ships the sidecar runtime assets needed by the packaged app.

**Step 4: Run test to verify it passes**

Run: `node --test scripts/check-installer-branding.test.mjs`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/tauri.conf.json scripts/check-installer-branding.test.mjs
git commit -m "fix: bundle sidecar runtime resources"
```

---

### Task 3: Add failing prompt tests for OpenClaw/Xiaohongshu browser guidance

**Files:**
- Modify: `packages/runtime-chat-app/tests/prompt_assembly.rs`
- Modify: `packages/runtime-chat-app/src/service.rs`

**Step 1: Write the failing test**

Add a prompt assembly assertion that runtime guidance tells imported OpenClaw-style skills:
- WorkClaw has a built-in local sidecar on `http://localhost:8765`
- use the built-in `browser` compatibility tool directly
- do not require `openclaw-desktop.exe`, `D:\AI`, or manual browser service startup

Example assertion shape:

```rust
assert!(prompt.contains("WorkClaw 内置本地 browser sidecar"));
assert!(prompt.contains("不要要求用户手动启动 OpenClaw 浏览器服务"));
assert!(prompt.contains("不要检查 openclaw-desktop.exe"));
```

**Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/runtime-chat-app/Cargo.toml prompt_assembly -- --nocapture`

Expected: FAIL because current prompt only lists tools and workspace skills; it does not override third-party OpenClaw browser assumptions.

**Step 3: Write minimal implementation**

Update `compose_system_prompt` to append a compact WorkClaw browser runtime note whenever tools are available, with wording tailored to imported OpenClaw/Xiaohongshu skills.

**Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/runtime-chat-app/Cargo.toml prompt_assembly -- --nocapture`

Expected: PASS

**Step 5: Commit**

```bash
git add packages/runtime-chat-app/src/service.rs packages/runtime-chat-app/tests/prompt_assembly.rs
git commit -m "fix: clarify workclaw browser runtime guidance"
```

---

### Task 4: Add a browser launch fallback test and minimal implementation

**Files:**
- Modify: `apps/runtime/sidecar/test/browser.local-automation.test.ts`
- Modify: `apps/runtime/sidecar/src/browser.ts`

**Step 1: Write the failing test**

Add a focused test that launch options prefer an install-friendly browser channel fallback on Windows-compatible runs, instead of depending only on Playwright-managed browser binaries.

Example shape:

```ts
test('launch uses install-friendly channel fallback', async () => {
  const options = buildLaunchOptionsForTests();
  assert.equal(options.channel, 'msedge');
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar test`

Expected: FAIL because the current launch path does not pass any install-friendly browser channel.

**Step 3: Write minimal implementation**

Update `apps/runtime/sidecar/src/browser.ts` to prefer a stable local channel such as `msedge` for packaged Windows runs while preserving existing development behavior where needed.

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar test`

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/browser.ts apps/runtime/sidecar/test/browser.local-automation.test.ts
git commit -m "fix: add packaged browser channel fallback"
```

---

### Task 5: Run end-to-end verification for the fixed chain

**Files:**
- Verify only

**Step 1: Run focused Rust sidecar tests**

Run: `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_sidecar -- --nocapture`

Expected: PASS

**Step 2: Run prompt assembly verification**

Run: `cargo test --manifest-path packages/runtime-chat-app/Cargo.toml prompt_assembly -- --nocapture`

Expected: PASS

**Step 3: Run sidecar/browser tests**

Run: `pnpm --dir apps/runtime/sidecar test`

Expected: PASS

**Step 4: Run installer config verification**

Run: `node --test scripts/check-installer-branding.test.mjs`

Expected: PASS

**Step 5: Summarize residual risk**

Call out any remaining gap explicitly, especially:
- whether the current bundle includes a real Node runtime or still depends on the target machine
- whether a fresh packaged installer was rebuilt and manually smoke-tested after the code change

