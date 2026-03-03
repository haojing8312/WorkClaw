# OpenClaw Employee UX Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver OpenClaw-native employee configuration with a user-friendly single identity field (`employee_id`), conversational `AGENTS.md/SOUL.md/USER.md` setup, and duplicate skill-name protection.

**Architecture:** Keep compatibility by introducing `employee_id` as the only UI-facing identity while mirroring to `role_id/openclaw_agent_id` in backend write paths. Add a new Tauri command module for conversational profile draft/apply and integrate it into employee UX as a guided chat wizard. Enforce skill display-name conflict checks in all install/import paths before writing `installed_skills`.

**Tech Stack:** React 18 + TypeScript + Vitest, Tauri 2 + Rust + sqlx(SQLite), existing OpenClaw vendor bridge and Feishu routing pipeline.

---

### Task 1: Add `employee_id` to domain model with compatibility mirror

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing Rust test**

```rust
#[tokio::test]
async fn upsert_employee_mirrors_employee_id_to_legacy_ids() {
    // upsert with employee_id = "project_manager"
    // assert stored role_id == employee_id
    // assert stored openclaw_agent_id == employee_id
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test test_im_employee_agents upsert_employee_mirrors_employee_id_to_legacy_ids -- --nocapture`  
Expected: FAIL due to missing `employee_id` field in structs/queries.

**Step 3: Implement minimal model change**

```rust
pub struct AgentEmployee {
    pub employee_id: String,
    // existing fields...
}
```

```ts
export interface AgentEmployee {
  employee_id: string;
  // existing fields...
}
```

Apply mirror logic in `upsert_agent_employee_with_pool`:
- normalized `employee_id`
- `role_id = employee_id`
- `openclaw_agent_id = employee_id`

**Step 4: Run test to verify it passes**

Run: `cargo test --test test_im_employee_agents upsert_employee_mirrors_employee_id_to_legacy_ids -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/types.ts apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m "feat(employee): add employee_id and mirror legacy identifiers"
```

---

### Task 2: Add DB migration/backfill for `employee_id`

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/tests/helpers/mod.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing migration test**

```rust
#[tokio::test]
async fn migration_backfills_employee_id_from_role_id() {
    // seed old row without employee_id, run migration, assert employee_id == role_id
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test test_im_employee_agents migration_backfills_employee_id_from_role_id -- --nocapture`  
Expected: FAIL because column/index/backfill is missing.

**Step 3: Implement migration**

Add in `db.rs`:
1. `ALTER TABLE agent_employees ADD COLUMN employee_id TEXT NOT NULL DEFAULT ''`
2. `UPDATE agent_employees SET employee_id = role_id WHERE TRIM(employee_id) = ''`
3. `CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_employees_employee_id_unique ON agent_employees(employee_id)`

Update test helper schema to include `employee_id`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test test_im_employee_agents migration_backfills_employee_id_from_role_id -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/helpers/mod.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m "feat(db): migrate and backfill employee_id for agent_employees"
```

---

### Task 3: Refactor EmployeeHub to single identity + 3-step UX

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`

**Step 1: Write failing frontend test**

```tsx
test("uses employee_id as only identity field and auto-generates value", async () => {
  // expect no role_id/openclaw input labels
  // expect employee_id input exists and auto-filled from name
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime test -- src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`  
Expected: FAIL due to old fields still visible.

**Step 3: Implement minimal UI refactor**

1. Replace `role_id` + `openclaw_agent_id` inputs with `employee_id` input.
2. Add 3-step sections:
   - 基础信息
   - 飞书连接
   - 技能与智能体配置
3. Auto-generate `employee_id` from name (slug + suffix on conflict).
4. Rename Feishu labels to `机器人 App ID/App Secret`.

**Step 4: Run test to verify it passes**

Run:
- `pnpm --dir apps/runtime test -- src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`
- `pnpm --dir apps/runtime test -- src/components/employees/__tests__/EmployeeHubView.risk-flow.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/App.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx
git commit -m "feat(ui): switch employee identity to employee_id and step-based form"
```

---

### Task 4: Keep Settings employee section consistent with new naming

**Files:**
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx`
- Test: `apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx`

**Step 1: Write/adjust failing tests**

```tsx
test("settings employee form uses employee_id and robot credential labels", async () => {
  // assert updated placeholders/labels
});
```

**Step 2: Run tests to verify failure**

Run: `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx src/components/__tests__/SettingsView.risk-flow.test.tsx`  
Expected: FAIL on outdated labels/fields.

**Step 3: Implement settings parity**

1. Show `员工编号(employee_id)` only.
2. Hide manual `role_id/openclaw_agent_id` editing.
3. Keep existing behavior via backend mirror.

**Step 4: Run tests to verify pass**

Run: `pnpm --dir apps/runtime test -- src/components/__tests__/SettingsView.feishu.test.tsx src/components/__tests__/SettingsView.risk-flow.test.tsx`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/SettingsView.tsx apps/runtime/src/components/__tests__/SettingsView.feishu.test.tsx apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx
git commit -m "refactor(settings): align employee fields with employee_id model"
```

---

### Task 5: Add backend commands for conversational OpenClaw profile files

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/agent_profile.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_agent_profile_docs.rs`

**Step 1: Write failing command tests**

```rust
#[tokio::test]
async fn apply_agent_profile_writes_agents_soul_user_files() {
    // call command with employee_id + answers
    // assert AGENTS.md/SOUL.md/USER.md files exist with expected sections
}
```

**Step 2: Run test to verify failure**

Run: `cargo test --test test_agent_profile_docs -- --nocapture`  
Expected: FAIL because module/commands do not exist.

**Step 3: Implement minimal command module**

Add commands:
1. `generate_agent_profile_draft(payload) -> { agents_md, soul_md, user_md }`
2. `apply_agent_profile(payload) -> { files: [{path, ok, error?}] }`

Write files into per-employee workspace:
`<employee_default_work_dir>/openclaw/<employee_id>/{AGENTS.md,SOUL.md,USER.md}`

**Step 4: Run tests to verify pass**

Run: `cargo test --test test_agent_profile_docs -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/agent_profile.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_agent_profile_docs.rs
git commit -m "feat(openclaw): add agent profile draft/apply commands"
```

---

### Task 6: Add conversational wizard UI for AGENTS/SOUL/USER generation

**Files:**
- Create: `apps/runtime/src/components/employees/AgentProfileChatWizard.tsx`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/employees/__tests__/AgentProfileChatWizard.test.tsx`

**Step 1: Write failing UI test**

```tsx
test("wizard asks one question at a time and applies markdown files", async () => {
  // mock invoke(generate/apply)
  // assert preview + apply success feedback
});
```

**Step 2: Run test to verify failure**

Run: `pnpm --dir apps/runtime test -- src/components/employees/__tests__/AgentProfileChatWizard.test.tsx`  
Expected: FAIL because component and invoke calls are missing.

**Step 3: Implement minimal wizard**

1. One-question-at-a-time conversational UI.
2. Local Q/A state.
3. Preview section for 3 markdown outputs.
4. Apply button + per-file result status.

**Step 4: Run tests to verify pass**

Run:
- `pnpm --dir apps/runtime test -- src/components/employees/__tests__/AgentProfileChatWizard.test.tsx`
- `pnpm --dir apps/runtime test -- src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/AgentProfileChatWizard.tsx apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/types.ts apps/runtime/src/components/employees/__tests__/AgentProfileChatWizard.test.tsx
git commit -m "feat(ui): add conversational openclaw agent profile wizard"
```

---

### Task 7: Enforce duplicate skill-name checks on install/import

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/skills.rs`
- Modify: `apps/runtime/src-tauri/src/commands/clawhub.rs`
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src-tauri/tests/test_skill_commands.rs`
- Test: `apps/runtime/src/components/__tests__/ChatView.find-skills-install.test.tsx`

**Step 1: Write failing backend test**

```rust
#[tokio::test]
async fn import_local_skill_rejects_duplicate_display_name() {
    // seed installed skill with same manifest.name
    // import another with same name, expect DUPLICATE_SKILL_NAME
}
```

**Step 2: Run test to verify failure**

Run: `cargo test --test test_skill_commands import_local_skill_rejects_duplicate_display_name -- --nocapture`  
Expected: FAIL because no name conflict guard exists.

**Step 3: Implement duplicate-name guard**

1. Add helper in `skills.rs`:
   - normalize display name
   - query installed manifests
   - return `DUPLICATE_SKILL_NAME:<name>` unless explicit replace flag
2. Reuse helper in `import_local_skill`, `install_skill`, `install_clawhub_skill`.
3. In UI, map this error to rename/override prompt.

**Step 4: Run tests to verify pass**

Run:
- `cargo test --test test_skill_commands -- --nocapture`
- `pnpm --dir apps/runtime test -- src/components/__tests__/ChatView.find-skills-install.test.tsx`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/skills.rs apps/runtime/src-tauri/src/commands/clawhub.rs apps/runtime/src/App.tsx apps/runtime/src-tauri/tests/test_skill_commands.rs apps/runtime/src/components/__tests__/ChatView.find-skills-install.test.tsx
git commit -m "feat(skills): prevent duplicate display-name conflicts on install"
```

---

### Task 8: Regression verification and docs sync

**Files:**
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/plans/2026-03-03-openclaw-employee-ux-design.md` (if minor deltas discovered during implementation)

**Step 1: Run frontend/sidecar/rust regression suite**

Run:
1. `pnpm --dir apps/runtime test`
2. `pnpm --dir apps/runtime/sidecar test`
3. `cargo test --test test_im_employee_agents --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

Expected: all PASS.

**Step 2: Update user docs**

Document:
1. `employee_id`-first model
2. conversational `AGENTS/SOUL/USER` config flow
3. duplicate skill-name conflict behavior

**Step 3: Run targeted smoke checks**

Run:
1. `pnpm --dir apps/runtime test -- src/components/employees/__tests__/AgentProfileChatWizard.test.tsx src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`
2. `cargo test --test test_agent_profile_docs -- --nocapture`

Expected: PASS.

**Step 4: Commit**

```bash
git add README.md README.zh-CN.md docs/plans/2026-03-03-openclaw-employee-ux-design.md
git commit -m "docs: describe employee_id model and conversational openclaw profile setup"
```

---

### Final Integration Checklist

1. Rebase/merge to `main` without dropping prior OpenClaw commits.  
2. Verify `git status` clean.  
3. Push branch and run CI.  
4. Manual product checks:
   - create employee using only `employee_id`
   - run conversational profile apply
   - verify generated markdown files on disk
   - send Feishu event and confirm routing/session continuity
   - install duplicate-name skill and validate conflict UX.

