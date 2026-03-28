# Runtime Contract Diagnostics Hygiene Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expand runtime contract coverage, add an OpenClaw-style hidden diagnostics summary entry point, and reduce recurring build warning noise without changing the ordinary chat experience.

**Architecture:** Keep one shared runtime observability summary builder in the Tauri diagnostics layer, reuse it for both export bundles and a hidden desktop-settings developer panel, and strengthen the runtime contract suite with three additional fixture-backed scenarios. Limit hygiene work to high-signal Rust warnings and lazy-loading of non-primary frontend scenes so the main chat path stays stable.

**Tech Stack:** Rust, Tauri commands, SQLx-backed runtime data, React 18, Vite, Vitest, pnpm, fixture-driven contract tests

---

### Task 1: Expand Runtime Contract Fixture Coverage

**Files:**
- Create: `apps/runtime/src-tauri/tests/fixtures/run_traces/compaction_overflow.json`
- Create: `apps/runtime/src-tauri/tests/fixtures/run_traces/failover_recovery.json`
- Create: `apps/runtime/src-tauri/tests/fixtures/run_traces/approval_reject.json`
- Modify: `apps/runtime/src-tauri/tests/test_runtime_contract.rs`
- Modify: `apps/runtime/src-tauri/tests/support/runtime_contract_testkit.rs`

**Step 1: Write the failing fixture-driven tests**

Add three new tests to `apps/runtime/src-tauri/tests/test_runtime_contract.rs` that mirror the existing six-case pattern:

```rust
#[tokio::test]
async fn runtime_contract_compaction_overflow_fixture_remains_stable() {
    let outcome = run_runtime_contract_fixture(RuntimeContractFixtureParams {
        fixture_name: "compaction_overflow",
        record_admission_conflict: false,
    })
    .await;

    assert_eq!(outcome.observability_snapshot["compaction"]["runs"], 1);
    assert!(matches!(outcome.trace_final_status.as_str(), "completed" | "failed" | "stopped"));
}
```

Repeat the same shape for `failover_recovery` and `approval_reject`.

**Step 2: Run the contract test binary to confirm the new cases fail**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_contract -- --nocapture
```

Expected: FAIL because the three new fixture files do not exist yet or the expected normalized trace does not match.

**Step 3: Author the new fixture JSON files**

Model each file after the existing fixtures in `apps/runtime/src-tauri/tests/fixtures/run_traces/`.

Use complete seeded event payloads for:

- `compaction_overflow`: run start, compaction-related event, terminal event
- `failover_recovery`: failed model attempt, retry/failover indicator, eventual success
- `approval_reject`: approval requested, approval rejected, terminal stopped/failed event

Keep the `expected` block fully normalized so `normalize_trace_for_fixture(...)` can compare byte-for-byte stable output.

**Step 4: Extend the contract support harness only if the new assertions need new fields**

If the new tests need richer assertions, extend `RuntimeContractOutcome` in `apps/runtime/src-tauri/tests/support/runtime_contract_testkit.rs` minimally, for example:

```rust
pub struct RuntimeContractOutcome {
    pub session_runs: Vec<SessionRunProjection>,
    pub normalized_trace: Value,
    pub trace_final_status: String,
    pub trace_child_session_parent: Option<String>,
    pub observability_snapshot: Value,
    pub recent_events: Vec<RuntimeObservedEvent>,
}
```

Do not add fields unless one of the new tests truly needs them.

**Step 5: Re-run the contract test binary**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_contract -- --nocapture
```

Expected: PASS with 9 fixture cases.

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/tests/fixtures/run_traces apps/runtime/src-tauri/tests/test_runtime_contract.rs apps/runtime/src-tauri/tests/support/runtime_contract_testkit.rs
git commit -m "test(runtime): expand contract trace fixtures"
```

### Task 2: Add Backend Runtime Observability Summary Builder

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle/types.rs`
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle/diagnostics_service.rs`
- Modify: `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs`

**Step 1: Write failing backend tests for the summary builder and export payload**

Add tests in `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs` for:

- summary builder emits compact counts and hints
- export bundle includes `runtime-diagnostics-summary.json`
- export bundle includes `runtime-diagnostics-summary.md`

Example assertion shape:

```rust
assert!(summary_markdown.contains("Admission conflicts: 2"));
assert!(summary_json.contains("\"conflicts\": 2"));
```

**Step 2: Run the targeted lib tests and confirm failure**

Run:

```bash
cargo test --lib desktop_lifecycle --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture
```

Expected: FAIL because the new summary builder and export fields are not implemented yet.

**Step 3: Add a stable summary payload type**

Extend `apps/runtime/src-tauri/src/commands/desktop_lifecycle/types.rs` with serializable summary structs, for example:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RuntimeDiagnosticsSummary {
    pub turns: RuntimeDiagnosticsTurnsSummary,
    pub admissions: RuntimeDiagnosticsAdmissionsSummary,
    pub guard_top_warning_kinds: Vec<RuntimeCountEntry>,
    pub failover_top_error_kinds: Vec<RuntimeCountEntry>,
    pub recent_event_preview: Vec<RuntimeDiagnosticEventPreview>,
    pub hints: Vec<String>,
}
```

Keep the summary compact and UI-oriented. Do not expose the full raw event payload here.

**Step 4: Implement a shared summary builder**

In `apps/runtime/src-tauri/src/commands/desktop_lifecycle/diagnostics_service.rs`, add a shared function such as:

```rust
pub(crate) fn build_runtime_diagnostics_summary(
    snapshot: &RuntimeObservabilitySnapshot,
    recent_events: &[RuntimeObservedEvent],
) -> RuntimeDiagnosticsSummary
```

The builder should:

- sort and trim top warning kinds
- sort and trim top failover error kinds
- produce a small recent-event preview, for example the last 10 entries
- add derived human-readable hints only when they are justified by the data

**Step 5: Reuse the summary in the export flow**

Update `DesktopDiagnosticsExportPayload` and `export_diagnostics_bundle(...)` so the zip includes:

- `runtime-diagnostics-summary.json`
- `runtime-diagnostics-summary.md`

Generate both from the same summary builder output.

**Step 6: Add a new Tauri command for the hidden frontend entry**

In `apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs`, add:

```rust
#[tauri::command]
pub async fn get_runtime_diagnostics_summary(
    app: AppHandle,
) -> Result<RuntimeDiagnosticsSummary, String>
```

It should read `RuntimeObservabilityState`, convert the raw snapshot plus recent events into the compact summary, and return that payload only.

**Step 7: Re-run backend tests**

Run:

```bash
cargo test --lib desktop_lifecycle --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture
```

Expected: PASS with the new summary and export coverage.

**Step 8: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/desktop_lifecycle.rs apps/runtime/src-tauri/src/commands/desktop_lifecycle/diagnostics_service.rs apps/runtime/src-tauri/src/commands/desktop_lifecycle/types.rs
git commit -m "feat(runtime): add diagnostics summary export"
```

### Task 3: Add Hidden Developer Diagnostics Summary to Desktop Settings

**Files:**
- Modify: `apps/runtime/src/components/settings/desktop/desktopSettingsService.ts`
- Modify: `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- Modify: `apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx`

**Step 1: Write the failing frontend test**

Extend `apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx` so it verifies:

- the new developer diagnostics section label exists
- the detailed summary is collapsed by default
- expanding it shows summary cards and recent-event preview

Mock a compact backend response:

```ts
if (command === "get_runtime_diagnostics_summary") {
  return Promise.resolve({
    turns: { active: 0, completed: 12, failed: 2, cancelled: 1 },
    admissions: { conflicts: 3 },
    guard_top_warning_kinds: [{ kind: "loop_detected", count: 2 }],
    failover_top_error_kinds: [{ kind: "network", count: 4 }],
    recent_event_preview: [{ kind: "session_run", event_type: "run_failed", run_id: "run-1" }],
    hints: ["Most recent failures were network-related."],
  });
}
```

**Step 2: Run the targeted Vitest file and confirm failure**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.data-retention.test.tsx --pool forks --poolOptions.forks.singleFork
```

Expected: FAIL because the service and UI do not fetch or render the summary yet.

**Step 3: Add the service call**

In `apps/runtime/src/components/settings/desktop/desktopSettingsService.ts`, add the typed client:

```ts
export async function getRuntimeDiagnosticsSummary() {
  return invoke<RuntimeDiagnosticsSummary>("get_runtime_diagnostics_summary");
}
```

Keep the payload type close to the Rust shape and avoid raw JSON strings in the UI contract.

**Step 4: Add a collapsed developer diagnostics block**

In `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`:

- add summary state and a collapsed boolean
- fetch the summary during diagnostics refresh or on first expansion
- render a default-collapsed section such as `开发者诊断摘要`
- show concise cards and a small recent-event list

Prefer a simple disclosure pattern over a custom complex panel.

**Step 5: Re-run the targeted frontend test**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.data-retention.test.tsx --pool forks --poolOptions.forks.singleFork
```

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/runtime/src/components/settings/desktop/desktopSettingsService.ts apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx apps/runtime/src/components/__tests__/SettingsView.data-retention.test.tsx
git commit -m "feat(runtime): add hidden developer diagnostics summary"
```

### Task 4: Reduce High-Signal Rust Warning Noise

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents/repo.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_policy.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs`
- Modify any additional warning-producing files proven by the current build output

**Step 1: Capture the current warning baseline**

Run:

```bash
cargo test --lib --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture
```

Expected: PASS with recurring `unused imports`, `dead_code`, and similar warnings.

**Step 2: Remove obviously over-broad production re-exports**

Shrink re-exports in `apps/runtime/src-tauri/src/commands/employee_agents/repo.rs` so only symbols used by production modules stay in unconditional exports. Move test-only types behind `#[cfg(test)]` if needed.

**Step 3: Move or wire test-only helpers intentionally**

In `apps/runtime/src-tauri/src/commands/chat_policy.rs`, either:

- mark truly test-only helpers with `#[cfg(test)]`, or
- wire them into live code if they are meant to be production behavior

Do not use blanket `#[allow(dead_code)]` for convenience.

**Step 4: Delete or consume unused runtime leftovers**

For leftovers like unused fields or helper functions in compaction and transcript-related modules, either:

- delete them if no longer needed, or
- route them into the new diagnostics summary if they now have a clear purpose

**Step 5: Re-run the lib test baseline**

Run:

```bash
cargo test --lib --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture
```

Expected: PASS with visibly fewer warnings than before.

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents/repo.rs apps/runtime/src-tauri/src/commands/chat_policy.rs apps/runtime/src-tauri/src/agent/runtime/compaction_pipeline.rs
git commit -m "refactor(runtime): reduce warning noise"
```

### Task 5: Reduce the Frontend Chunk Warning with Scene Lazy Loading

**Files:**
- Modify: `apps/runtime/src/components/AppMainContent.tsx`
- Modify: `apps/runtime/vite.config.ts`
- Add tests only if the lazy-loading path needs dedicated coverage

**Step 1: Confirm the current production build warning**

Run:

```bash
pnpm --dir apps/runtime build
```

Expected: PASS with a Vite warning about chunks larger than 500 kB after minification.

**Step 2: Convert non-primary scenes to lazy imports**

In `apps/runtime/src/components/AppMainContent.tsx`, replace direct imports with `React.lazy(...)` for:

- `SettingsView`
- `ExpertsView`
- `ExpertCreateView`
- `EmployeeHubScene`
- `PackagingView`

Wrap those branches in `Suspense` with a simple existing-style loading placeholder:

```tsx
const SettingsView = React.lazy(() => import("./SettingsView"));

<Suspense fallback={<div className="flex items-center justify-center h-full sm-text-muted text-sm">加载中...</div>}>
  <SettingsView ... />
</Suspense>
```

Do not lazy-load `ChatView` in this round.

**Step 3: Only tune Vite config if warning remains after real code-splitting**

If a build still warns after lazy-loading, add the smallest possible bundler hint in `apps/runtime/vite.config.ts`, for example a minimal `manualChunks` split for one obviously heavy dependency. Avoid broad manual chunk maps unless measurement proves they are necessary.

**Step 4: Re-run the frontend build**

Run:

```bash
pnpm --dir apps/runtime build
```

Expected: PASS with the chunk warning removed or materially reduced.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/AppMainContent.tsx apps/runtime/vite.config.ts
git commit -m "perf(runtime): split non-primary frontend scenes"
```

### Task 6: Final Verification and Packaging Sanity

**Files:**
- Verify only; no new file targets expected unless fixes are required

**Step 1: Run the runtime contract suite**

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_runtime_contract -- --nocapture
```

Expected: PASS with 9 fixture tests.

**Step 2: Run the Tauri lib suite**

```bash
cargo test --lib --manifest-path apps/runtime/src-tauri/Cargo.toml -- --nocapture
```

Expected: PASS.

**Step 3: Run the Rust fast path**

```bash
pnpm test:rust-fast
```

Expected: PASS.

**Step 4: Run the targeted frontend diagnostics test**

```bash
pnpm --dir apps/runtime exec vitest run src/components/__tests__/SettingsView.data-retention.test.tsx --pool forks --poolOptions.forks.singleFork
```

Expected: PASS.

**Step 5: Run the frontend production build**

```bash
pnpm --dir apps/runtime build
```

Expected: PASS with improved or eliminated chunk warning.

**Step 6: Run desktop packaging sanity**

```bash
$env:PNPM_STORE_DIR='D:\code\WorkClaw\.pnpm-store-local'; $env:npm_config_store_dir='D:\code\WorkClaw\.pnpm-store-local'; pnpm build:runtime
```

Expected: PASS and regenerated `MSI` / `NSIS` outputs.

**Step 7: Run final diff hygiene**

```bash
git diff --check
```

Expected: PASS.

**Step 8: Commit or stop for review**

If all verification is green:

```bash
git status --short
```

Then either create a final integration commit or stop for human review, depending on the active execution workflow.
