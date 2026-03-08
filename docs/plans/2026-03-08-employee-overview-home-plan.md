# Employee Overview Home Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn the `智能体员工` page into a true overview home that shows employee/team health first and moves detailed configuration behind internal tabs.

**Architecture:** Keep `EmployeeHubView` as the shell for this phase, introduce an internal `overview | employees | teams | runs | settings` tab model, and extract the current long-form sections into focused panels. Reuse current employee/team state where possible, and add one explicit recent-runs query path so the overview is not driven by temporary UI state.

**Tech Stack:** React, TypeScript, Tauri, Rust, Vitest, Cargo tests

---

### Task 1: Lock the new overview behavior with failing UI tests

**Files:**
- Create: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-creator-entry.test.tsx`

**Step 1: Write the failing test**

Add a new overview-focused test file that renders `EmployeeHubView` and asserts:

```tsx
expect(screen.getByRole("heading", { name: "智能体员工" })).toBeInTheDocument();
expect(screen.getByRole("tab", { name: "总览" })).toHaveAttribute("aria-selected", "true");
expect(screen.getByText("员工总数")).toBeInTheDocument();
expect(screen.getByText("团队总数")).toBeInTheDocument();
expect(screen.queryByText("拉群协作（最多 10 人）")).not.toBeInTheDocument();
expect(screen.queryByText("员工详情")).not.toBeInTheDocument();
```

Update existing tests so team orchestration assertions navigate to the `团队` tab first instead of assuming the form is on the default screen.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/employees/__tests__/EmployeeHubView.employee-creator-entry.test.tsx
```

Expected: FAIL because the current page still renders orchestration and employee detail sections on the default screen.

**Step 3: Write minimal implementation**

In `apps/runtime/src/components/employees/EmployeeHubView.tsx`:

- add internal tab state:

```ts
type EmployeeHubTab = "overview" | "employees" | "teams" | "runs" | "settings";
const [activeTab, setActiveTab] = useState<EmployeeHubTab>("overview");
```

- add a top navigation bar for the five tabs
- render the current “拉群协作”“员工列表/详情”“长期记忆”“默认工作目录” blocks only inside non-overview tabs

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/employees/__tests__/EmployeeHubView.employee-creator-entry.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-creator-entry.test.tsx
git commit -m "feat: add employee hub overview tab"
```

### Task 2: Add reusable overview summary and pending-issue computation

**Files:**
- Create: `apps/runtime/src/components/employees/employeeHubOverview.ts`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx`

**Step 1: Write the failing test**

Extend the overview test to assert computed summary values and pending issues:

```tsx
expect(screen.getByTestId("employee-overview-metric-employees")).toHaveTextContent("3");
expect(screen.getByTestId("employee-overview-metric-teams")).toHaveTextContent("2");
expect(screen.getByText("1 名员工未完成连接配置")).toBeInTheDocument();
expect(screen.getByText("1 个团队角色不完整")).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: FAIL because no metric/pending aggregation helpers exist.

**Step 3: Write minimal implementation**

Create `apps/runtime/src/components/employees/employeeHubOverview.ts` with pure helpers like:

```ts
export function buildEmployeeHubMetrics(input: { employees: Employee[]; groups: EmployeeGroup[]; runs: EmployeeGroupRunSummary[] }) { ... }
export function buildEmployeeHubPendingItems(input: { employees: Employee[]; groups: EmployeeGroup[] }) { ... }
```

Keep rules minimal and explicit:

- employee available = enabled and has employee id and either primary skill or default assistant fallback
- pending connection = employee has any Feishu identifier intent but no valid app pair
- incomplete team = missing entry or coordinator

In `EmployeeHubView.tsx`, call these helpers for overview rendering instead of mixing the logic directly into JSX.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/employeeHubOverview.ts apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
git commit -m "feat: add employee hub overview metrics"
```

### Task 3: Move detailed page sections behind focused tabs

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.memory-governance.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx`
- Modify: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx`

**Step 1: Write the failing test**

Add assertions that:

- `员工` tab contains employee list/detail and employee actions
- `团队` tab contains team creation/template/run controls
- `设置` tab contains the global default work directory block
- `总览` tab does not contain any of the above

Example:

```tsx
fireEvent.click(screen.getByRole("tab", { name: "设置" }));
expect(screen.getByText("全局默认工作目录")).toBeInTheDocument();

fireEvent.click(screen.getByRole("tab", { name: "总览" }));
expect(screen.queryByText("长期记忆管理")).not.toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.memory-governance.test.tsx src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx
```

Expected: FAIL because the current page renders everything together.

**Step 3: Write minimal implementation**

In `apps/runtime/src/components/employees/EmployeeHubView.tsx`:

- keep existing blocks, but wrap them in tab-specific containers
- recommended split:
  - `overview` = metrics, pending items, employee summary, team summary, recent runs, shortcuts
  - `employees` = employee list, employee detail, Feishu, profile, memory
  - `teams` = team creation, templates, team cards, run start
  - `runs` = recent run list and jump actions
  - `settings` = global default work dir and future system settings

Do not refactor more than necessary in this task; the point is clear boundary, not deep component extraction.

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.memory-governance.test.tsx src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.memory-governance.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx
git commit -m "refactor: separate employee hub detail tabs"
```

### Task 4: Add a stable recent-runs query for the overview and runs tab

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Test: `apps/runtime/src-tauri/tests/test_employee_groups_db.rs`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx`

**Step 1: Write the failing test**

Add a backend test in `apps/runtime/src-tauri/tests/test_employee_groups_db.rs` that creates group runs and expects a list command to return the latest items in descending order:

```rust
assert_eq!(runs.len(), 2);
assert_eq!(runs[0].status, "running");
assert_eq!(runs[1].status, "completed");
```

Extend the overview UI test to expect a recent run row:

```tsx
expect(screen.getByText("最近运行")).toBeInTheDocument();
expect(screen.getByText("复杂任务拆解")).toBeInTheDocument();
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_employee_groups_db
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: FAIL because there is no stable recent-runs list command for the employee hub.

**Step 3: Write minimal implementation**

In `apps/runtime/src-tauri/src/commands/employee_agents.rs`:

- add a lightweight command, for example:

```rust
#[tauri::command]
pub async fn list_employee_group_runs(limit: Option<i64>) -> Result<Vec<EmployeeGroupRunSummary>, String> { ... }
```

- query the latest rows from the existing group run storage
- return only fields needed by the overview/runs tab

In `apps/runtime/src-tauri/src/lib.rs`, register the new command.

In `apps/runtime/src/types.ts`, add the matching frontend type:

```ts
export interface EmployeeGroupRunSummary {
  id: string;
  group_id: string;
  group_name: string;
  goal: string;
  status: string;
  started_at?: string;
  finished_at?: string;
  session_id?: string;
}
```

In `EmployeeHubView.tsx`, load this data on mount/refresh and use it for both the `最近运行` overview block and the dedicated `运行` tab.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_employee_groups_db
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src/types.ts apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src-tauri/tests/test_employee_groups_db.rs apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
git commit -m "feat: add employee hub recent runs summary"
```

### Task 5: Wire overview cards and pending items to actionable tab filters

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/employeeHubOverview.ts`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx`

**Step 1: Write the failing test**

Add assertions for navigation behavior:

```tsx
fireEvent.click(screen.getByTestId("employee-overview-metric-employees"));
expect(screen.getByRole("tab", { name: "员工" })).toHaveAttribute("aria-selected", "true");

fireEvent.click(screen.getByText("1 个团队角色不完整"));
expect(screen.getByRole("tab", { name: "团队" })).toHaveAttribute("aria-selected", "true");
```

If you add local filter state, assert the expected filter chip or empty-state label too.

**Step 2: Run test to verify it fails**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: FAIL because overview cards are not actionable yet.

**Step 3: Write minimal implementation**

In `EmployeeHubView.tsx`:

- add local drilldown state:

```ts
type EmployeeHubDrilldown =
  | { tab: "employees"; filter: "all" | "available" | "pending-connection" }
  | { tab: "teams"; filter: "all" | "incomplete" | "failed" }
  | { tab: "runs"; filter: "all" | "running" | "failed" }
  | null;
```

- clicking a metric or pending item should:
  - switch tab
  - set the matching local filter

- keep the first version simple: a small text filter label above the list is enough; no complex faceted filtering UI

**Step 4: Run test to verify it passes**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
```

Expected: PASS

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/components/employees/employeeHubOverview.ts apps/runtime/src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx
git commit -m "feat: add employee hub overview drilldowns"
```

### Task 6: Final regression and manual verification

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/components/employees/employeeHubOverview.ts`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Test: targeted suites below

**Step 1: Run targeted frontend regression**

Run:

```bash
pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.overview-home.test.tsx src/components/employees/__tests__/EmployeeHubView.group-orchestrator.test.tsx src/components/employees/__tests__/EmployeeHubView.team-template.test.tsx src/components/employees/__tests__/EmployeeHubView.memory-governance.test.tsx src/components/employees/__tests__/EmployeeHubView.feishu-connection-status.test.tsx src/components/employees/__tests__/EmployeeHubView.employee-creator-entry.test.tsx src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx
```

Expected: PASS

**Step 2: Run targeted backend regression**

Run:

```bash
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_employee_groups_db
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --test test_im_employee_agents
```

Expected: PASS

**Step 3: Manual verification**

- Open `智能体员工`, confirm default screen is `总览`
- Confirm first screen shows metrics and pending items without the team form
- Click `员工总数`, `团队总数`, and a pending item, verify tab drilldown works
- Open `团队`, verify existing team create/template/run workflows still work
- Open `员工`, verify employee detail, Feishu config, profile preview, and memory tools still work
- Open `设置`, verify global default work dir still saves
- Open `运行`, verify recent runs list can jump to the related session

**Step 4: Final commit**

```bash
git add apps/runtime/src apps/runtime/src-tauri/src apps/runtime/src-tauri/tests docs/plans
git commit -m "feat: turn employee hub into an overview home"
```
