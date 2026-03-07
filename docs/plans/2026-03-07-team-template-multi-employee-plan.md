# Team Template Multi-Employee Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a configurable team-template system that seeds a default multi-employee team on first launch and upgrades group runs from simulated summaries to real employee collaboration with review loops.

**Architecture:** Add a built-in team-template layer and a first-run bootstrapper in the Tauri runtime, extend the existing employee/group/run schema with rules and events, and replace the current simulated group orchestrator with a runtime that dispatches real employee-scoped steps. Reuse the existing employee/session/memory foundations, expose the new structures to the React UI, and keep the default “三省六部” team as a template instance rather than a special-case code path.

**Tech Stack:** Rust (Tauri, sqlx, SQLite, serde), React + TypeScript, Vitest, existing WorkClaw employee/chat/task runtime.

---

### Task 1: Prepare Isolated Workspace And Baseline

**Files:**
- Check: `.worktrees/`
- Check: `.gitignore`
- Verify: `apps/runtime/src-tauri/Cargo.toml`
- Verify: `apps/runtime/package.json`

**Step 1: Create an isolated worktree**

Run:

```bash
git check-ignore -q .worktrees
git worktree add .worktrees/team-template-v1 -b feat/team-template-v1
```

Expected: worktree created at `.worktrees/team-template-v1`.

**Step 2: Install project dependencies in the worktree**

Run:

```bash
pnpm install
```

Expected: workspace dependencies install without lockfile drift.

**Step 3: Verify Rust test baseline**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
```

Expected: baseline tests pass before feature changes.

**Step 4: Verify UI test baseline**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/__tests__/ChatView.im-routing-panel.test.tsx
```

Expected: existing group/orchestration UI tests pass before modifications.

**Step 5: Commit the worktree setup note if `.gitignore` changed**

Run:

```bash
git add .gitignore
git commit -m chore(repo):ignore-worktree-directory
```

Expected: only needed if `.worktrees` was not ignored.

### Task 2: Define Built-In Team Template Schema And Loader

**Files:**
- Create: `apps/runtime/src-tauri/builtin-team-templates/sansheng-liubu.json`
- Create: `apps/runtime/src-tauri/src/team_templates.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs`

**Step 1: Write the failing loader test**

Add a test that proves the built-in template parses and exposes employees, roles, and rules.

```rust
#[tokio::test]
async fn builtin_team_template_loads_default_sansheng_liubu() {
    let template = runtime_lib::team_templates::load_builtin_template("sansheng-liubu")
        .expect("template should load");
    assert_eq!(template.template_id, "sansheng-liubu");
    assert!(template.seed_on_first_run);
    assert!(template.employees.iter().any(|e| e.employee_id == "taizi"));
    assert!(template.rules.iter().any(|r| r.relation_type == "review"));
}
```

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap builtin_team_template_loads_default_sansheng_liubu -- --nocapture
```

Expected: FAIL because the loader/module does not exist yet.

**Step 3: Add the template file and loader**

Create a serializable schema that includes:

```rust
pub struct TeamTemplate {
    pub template_id: String,
    pub template_version: String,
    pub seed_on_first_run: bool,
    pub name: String,
    pub description: String,
    pub default_entry_employee_key: String,
    pub employees: Vec<TeamTemplateEmployee>,
    pub rules: Vec<TeamTemplateRule>,
}
```

And load `sansheng-liubu.json` via `include_str!`.

**Step 4: Run the test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap builtin_team_template_loads_default_sansheng_liubu -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/builtin-team-templates/sansheng-liubu.json apps/runtime/src-tauri/src/team_templates.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs
git commit -m feat(runtime):add-built-in-team-template-loader
```

### Task 3: Add Database Support For Template Seeds, Team Rules, And Run Events

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Test: `apps/runtime/src-tauri/tests/test_employee_groups_db.rs`
- Test: `apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs`

**Step 1: Write the failing schema tests**

Add assertions that the new tables and columns exist and are writable.

```rust
#[tokio::test]
async fn db_creates_team_rule_and_run_event_tables() {
    let pool = helpers::setup_test_pool().await;
    sqlx::query("INSERT INTO employee_group_rules (id, group_id, from_employee_id, to_employee_id, relation_type, phase_scope, required, priority, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind("rule-1")
        .bind("group-1")
        .bind("zhongshu")
        .bind("menxia")
        .bind("review")
        .bind("plan")
        .bind(1)
        .bind(100)
        .bind("2026-03-07T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert rule");
}
```

**Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_employee_groups_db -- --nocapture
```

Expected: FAIL because the schema objects are missing.

**Step 3: Extend the schema and migrations**

Add:

- `employee_group_rules`
- `group_run_events`
- `seeded_team_templates`

And add new columns to:

- `employee_groups`
- `group_runs`
- `group_run_steps`

Keep migrations idempotent with `ALTER TABLE ... ADD COLUMN` guarded by best-effort execution.

**Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_employee_groups_db -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap -- --nocapture
```

Expected: PASS for both suites.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/test_employee_groups_db.rs apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs
git commit -m feat(runtime):add-team-template-rule-and-event-schema
```

### Task 4: Seed The Default Team Template On First Launch

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/team_templates.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/commands/agent_profile.rs`
- Test: `apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs`

**Step 1: Write the failing bootstrap test**

Add a test that starts with an empty DB and verifies that the template creates employees, skills, profiles, group, and rules once.

```rust
#[tokio::test]
async fn first_run_bootstrap_seeds_default_team_once() {
    let (pool, app, tmp) = helpers::setup_bootstrap_app().await;
    runtime_lib::team_templates::seed_builtin_team_templates(&pool, &app)
        .await
        .expect("seed");
    runtime_lib::team_templates::seed_builtin_team_templates(&pool, &app)
        .await
        .expect("seed twice");

    let employees: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agent_employees")
        .fetch_one(&pool)
        .await
        .expect("count employees");
    assert!(employees.0 >= 7);
}
```

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap first_run_bootstrap_seeds_default_team_once -- --nocapture
```

Expected: FAIL because the seeding API does not exist.

**Step 3: Implement bootstrap seeding**

Add a bootstrap entry point that:

- checks `seeded_team_templates`
- loads seedable built-in templates
- upserts employees using existing employee APIs
- writes `AGENTS.md`, `SOUL.md`, `USER.md`
- creates a default `employee_group`
- inserts `employee_group_rules`
- records the seed operation

Use existing employee/profile helpers instead of duplicating creation logic.

**Step 4: Run the bootstrap tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap -- --nocapture
```

Expected: PASS with idempotent seeding.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/team_templates.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/commands/agent_profile.rs apps/runtime/src-tauri/tests/test_team_templates_bootstrap.rs
git commit -m feat(runtime):seed-default-team-template-on-first-run
```

### Task 5: Replace Simulated Group Runs With Real Run Planning And Event Persistence

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/group_orchestrator.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`
- Test: `apps/runtime/src-tauri/tests/test_orchestrator_interrupt_priority.rs`

**Step 1: Write the failing orchestration test**

Add a test that starts a run and verifies that planning creates persisted steps/events instead of only simulated outputs.

```rust
#[tokio::test]
async fn start_group_run_persists_plan_steps_and_events() {
    let pool = helpers::setup_group_run_db().await;
    let result = runtime_lib::commands::employee_agents::start_employee_group_run_with_pool(
        &pool,
        helpers::start_group_run_input("group-1", "实现复杂协作"),
    )
    .await
    .expect("start run");

    assert!(result.steps.iter().any(|s| s.step_type == "plan"));
    let events: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM group_run_events WHERE run_id = ?")
        .bind(&result.run_id)
        .fetch_one(&pool)
        .await
        .expect("count events");
    assert!(events.0 > 0);
}
```

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents start_group_run_persists_plan_steps_and_events -- --nocapture
```

Expected: FAIL because the run output is still simulated execute-only steps.

**Step 3: Implement the real run skeleton**

Refactor `group_orchestrator.rs` to return a structured plan state machine instead of a single simulated summary:

```rust
pub struct GroupRunPlan {
    pub current_phase: String,
    pub steps: Vec<GroupRunStepDraft>,
    pub events: Vec<GroupRunEventDraft>,
}
```

Persist:

- `plan` steps
- initial `review` placeholder when required
- `run_created` / `phase_started` / `step_created` events

Do not execute employee sessions yet; this task only removes the fake report architecture.

**Step 4: Run the tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_orchestrator_interrupt_priority -- --nocapture
```

Expected: PASS with real persisted plan skeleton and no regressions.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/src/agent/group_orchestrator.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs apps/runtime/src-tauri/tests/test_orchestrator_interrupt_priority.rs
git commit -m feat(runtime):persist-real-group-run-plan-skeleton
```

### Task 6: Execute Steps In Real Employee Contexts

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/task_tool.rs`
- Test: `apps/runtime/src-tauri/tests/test_task_tool.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing employee-targeted execution test**

Add a test that proves a run step uses the target employee’s real skill/work_dir/memory context.

```rust
#[tokio::test]
async fn execute_group_step_uses_target_employee_context() {
    let pool = helpers::setup_employee_execution_db().await;
    let step = helpers::seed_execute_step(&pool, "bingbu");
    let outcome = runtime_lib::commands::employee_agents::run_group_step_with_pool(&pool, &step.id)
        .await
        .expect("execute step");
    assert_eq!(outcome.assignee_employee_id, "bingbu");
    assert!(!outcome.session_id.is_empty());
}
```

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents execute_group_step_uses_target_employee_context -- --nocapture
```

Expected: FAIL because no employee-targeted run API exists.

**Step 3: Add real employee step execution**

Implement a runtime helper that:

- resolves `target_employee_id`
- creates/reuses a session for the employee within the run
- builds the system prompt from the employee’s profile + skill
- executes the step through the existing chat/agent executor path
- records step outputs and run events

Keep `TaskTool` available for ad hoc sub-agent use, but make team orchestration call the formal employee-step runner.

**Step 4: Run the execution tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_task_tool -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_multi_role_e2e -- --nocapture
```

Expected: PASS with real employee execution and no regression in existing delegation flows.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/agent/tools/task_tool.rs apps/runtime/src-tauri/tests/test_task_tool.rs apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m feat(runtime):execute-group-steps-in-employee-context
```

### Task 7: Add Hard Review / Reject / Retry / Reassign Run Control

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/group_orchestrator.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing review-loop test**

```rust
#[tokio::test]
async fn hard_review_reject_moves_run_back_to_previous_phase() {
    let pool = helpers::setup_reviewable_run_db().await;
    let run_id = helpers::seed_review_phase_run(&pool).await;
    runtime_lib::commands::employee_agents::review_group_run_step_with_pool(
        &pool,
        &run_id,
        "reject",
        "缺少回滚方案",
    )
    .await
    .expect("reject");

    let snapshot = runtime_lib::commands::employee_agents::get_group_run_by_id_with_pool(&pool, &run_id)
        .await
        .expect("load snapshot");
    assert_eq!(snapshot.current_phase, "plan");
    assert_eq!(snapshot.review_round, 1);
}
```

**Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents hard_review_reject_moves_run_back_to_previous_phase -- --nocapture
```

Expected: FAIL because there is no formal review action yet.

**Step 3: Implement review control**

Add commands/helpers for:

- approve
- reject
- retry failed step
- reassign failed step
- pause / resume run

Persist:

- updated phase
- incremented review round
- new revision step/event rows

**Step 4: Run the tests**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents -- --nocapture
```

Expected: PASS with review loops and run control APIs.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src-tauri/src/agent/group_orchestrator.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m feat(runtime):add-review-loop-and-run-controls
```

### Task 8: Expose Team Templates, Rules, And Runs In The Employee Hub UI

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx`
- Create: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx`

**Step 1: Write the failing UI tests**

Add a test for:

- showing the seeded default team
- rendering members by role
- displaying rule summaries

```tsx
it("shows the seeded default team template instance with coordinator and reviewer roles", async () => {
  invokeMock.mockImplementation((command) => {
    if (command === "list_employee_groups") {
      return Promise.resolve([{ id: "group-sansheng", name: "默认复杂任务团队", coordinator_employee_id: "shangshu", member_employee_ids: ["taizi", "zhongshu", "menxia", "shangshu"] }]);
    }
    if (command === "list_employee_group_rules") {
      return Promise.resolve([{ from_employee_id: "zhongshu", to_employee_id: "menxia", relation_type: "review" }]);
    }
    return Promise.resolve([]);
  });
  render(<EmployeeHubView ... />);
  expect(screen.getByText("默认复杂任务团队")).toBeInTheDocument();
  expect(screen.getByText(/review/i)).toBeInTheDocument();
});
```

**Step 2: Run the tests to verify they fail**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx
```

Expected: FAIL because the data types and UI sections do not exist yet.

**Step 3: Implement the UI shape**

Add frontend types for:

- team template instance metadata
- group rules
- run event summaries

Then update `EmployeeHubView` to show:

- seeded team banner
- team members by role
- rule summary list
- start run entry tied to the team instance

**Step 4: Run the UI tests**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/App.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx
git commit -m feat(ui):show-seeded-team-template-and-rules
```

### Task 9: Render Real Run Phase / Steps / Review State In ChatView

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`

**Step 1: Write the failing board test**

Add a test that consumes a real run snapshot with phases, review state, and event summaries.

```tsx
it("renders current phase, review round, and review-blocked steps from backend snapshot", async () => {
  invokeMock.mockImplementation((command) => {
    if (command === "get_employee_group_run_snapshot") {
      return Promise.resolve({
        run_id: "run-1",
        state: "waiting_review",
        current_phase: "review",
        review_round: 2,
        steps: [
          { id: "step-plan", assignee_employee_id: "zhongshu", status: "completed", step_type: "plan" },
          { id: "step-review", assignee_employee_id: "menxia", status: "blocked", step_type: "review" }
        ],
        events: []
      });
    }
    return Promise.resolve(null);
  });
  render(<ChatView sessionId="session-review" ... />);
  expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("阶段：审核");
  expect(screen.getByTestId("group-orchestration-board")).toHaveTextContent("第 2 轮");
});
```

**Step 2: Run the test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.im-routing-panel.test.tsx
```

Expected: FAIL because the board shape does not yet support phase/review/event details.

**Step 3: Update ChatView**

Render:

- current phase
- review round
- assignee per step
- status chips
- recent event list
- wait/review indicators

Keep compatibility with existing snapshots during rollout.

**Step 4: Run the tests**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.im-routing-panel.test.tsx
```

Expected: PASS.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx
git commit -m feat(ui):render-real-team-run-phase-and-review-state
```

### Task 10: Add User-Created Team Template Flows And Final Documentation

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx`
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/architecture/employee-identity-model.md`

**Step 1: Write the failing UI flow test**

Add a test for copying the default team into a user-owned variant.

```tsx
it("can clone the default seeded team into a custom team instance", async () => {
  invokeMock.mockResolvedValue("group-custom");
  render(<EmployeeHubView ... />);
  fireEvent.click(screen.getByTestId("employee-team-clone-group-sansheng"));
  expect(invokeMock).toHaveBeenCalledWith("clone_employee_group_template", expect.anything());
});
```

**Step 2: Run the test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx
```

Expected: FAIL because clone/create-from-template flows are missing.

**Step 3: Implement the minimal template-copy flow and docs**

Add:

- clone/create-from-template entry in `EmployeeHubView`
- command wiring if needed
- README/user-facing docs that explain:
  - first-run default team
  - template vs instance
  - how users create their own teams

**Step 4: Run the final verification**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_team_templates_bootstrap --test test_employee_groups_db --test test_im_employee_agents --test test_task_tool --test test_im_multi_role_e2e -- --nocapture
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/__tests__/ChatView.im-routing-panel.test.tsx
```

Expected: PASS for all listed test suites.

**Step 5: Commit**

Run:

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx README.md README.en.md docs/architecture/employee-identity-model.md
git commit -m docs+feat:document-and-expose-team-template-clone-flow
```
