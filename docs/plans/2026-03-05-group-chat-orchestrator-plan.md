# Group Chat Orchestrator (10 Members) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在桌面端实现“拉群协作”：最多 10 名员工、协调员轮次调度、自动完成计划+执行+汇报闭环。

**Architecture:** 在现有主/子员工委托能力上增加 Group Orchestrator 状态机与群组数据模型。前端新增拉群入口与协作看板；后端新增 run/step 编排与失败兜底；任务执行仍复用 `task` 工具。

**Tech Stack:** Rust (Tauri + sqlx + SQLite), React + TypeScript + Vitest, existing agent/task runtime.

---

### Task 1: 建立群组与编排数据表

**Files:**
- Modify: `apps/runtime/src-tauri/migrations/*`（新增 migration）
- Modify: `apps/runtime/src-tauri/src/db.rs`（如有集中迁移入口）
- Test: `apps/runtime/src-tauri/tests/test_employee_agents_db.rs`

**Step 1: Write the failing test**

新增测试断言可创建/读取：
- `employee_groups`
- `group_runs`
- `group_run_steps`

并校验 `member_employee_ids_json` 上限约束逻辑（最多 10）。

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_employee_agents_db -- --nocapture`  
Expected: FAIL（表不存在或字段缺失）。

**Step 3: Write minimal implementation**

新增 migration，创建三张表与基础索引（`group_id`, `run_id`, `state`）。

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_employee_agents_db -- --nocapture`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/migrations apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/tests/test_employee_agents_db.rs
git commit -m "feat(group): add group orchestrator persistence tables"
```

### Task 2: 后端群组管理命令（创建/查询/校验 10 人上限）

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`（注册 tauri command）
- Test: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing test**

增加命令级测试：
- 创建群组成功（含 coordinator）
- 成员数 >10 返回明确错误
- 协调员必须属于成员列表

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_employee_agents -- --nocapture`  
Expected: FAIL。

**Step 3: Write minimal implementation**

新增 command：
- `create_employee_group`
- `list_employee_groups`
- `delete_employee_group`

并写入参数校验。

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_employee_agents -- --nocapture`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m "feat(group): add employee group CRUD with member constraints"
```

### Task 3: Group Orchestrator 状态机（计划/执行/汇报）

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/group_orchestrator.rs`
- Modify: `apps/runtime/src-tauri/src/agent/mod.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs`

**Step 1: Write the failing test**

新增 E2E 用例断言一次 run 必经：
- `planning`
- `executing`
- `synthesizing`
- `done|failed`

且最终消息含“计划摘要 + 执行结果 + 汇报”。

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: FAIL。

**Step 3: Write minimal implementation**

实现 orchestrator：
- 计划阶段由协调员生成结构化 steps
- 执行阶段按轮次分发到成员（复用 `task`）
- 汇总阶段统一产出 final report

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/group_orchestrator.rs apps/runtime/src-tauri/src/agent/mod.rs apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs
git commit -m "feat(orchestrator): add coordinator-led group run state machine"
```

### Task 4: 轮次调度与并发窗口（默认并发 3）

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/group_orchestrator.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs`

**Step 1: Write the failing test**

新增测试断言：
- 成员按轮次收到任务
- 同时活跃任务数不超过并发窗口

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: FAIL。

**Step 3: Write minimal implementation**

实现轮次队列和并发窗口：
- `round_no` 递增
- 窗口满时排队
- 完成/失败释放槽位

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/group_orchestrator.rs apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs
git commit -m "feat(orchestrator): enforce round-robin scheduling with concurrency window"
```

### Task 5: 失败兜底（超时重试 1 次 + 降级汇报）

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/group_orchestrator.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs`

**Step 1: Write the failing test**

新增测试：
- 单成员超时后重试 1 次
- 重试失败标记 `failed`，流程继续
- 最终报告包含未完成项与补救建议

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: FAIL。

**Step 3: Write minimal implementation**

加入：
- step 超时计时
- retry_count 控制
- 失败降级摘要模板

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_multi_role_e2e -- --nocapture`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/group_orchestrator.rs apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/tests/test_im_multi_role_e2e.rs
git commit -m "feat(orchestrator): add timeout retry and degraded final reporting"
```

### Task 6: 前端“拉群”入口与群组管理

**Files:**
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/App.tsx`
- Test: `apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`

**Step 1: Write the failing test**

新增 UI 测试：
- 可多选成员创建群组
- 超过 10 人时阻止提交并提示
- 可选择协调员

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`  
Expected: FAIL。

**Step 3: Write minimal implementation**

新增群组管理区：
- 创建群组表单（名称、成员、协调员）
- 群组列表与删除
- 调用新 tauri commands

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src/types.ts apps/runtime/src/App.tsx apps/runtime/src/components/employees/__tests__/EmployeeHubView.employee-id-flow.test.tsx
git commit -m "feat(ui): add employee group creation with coordinator selection"
```

### Task 7: 聊天页协作看板（阶段/轮次/成员状态）

**Files:**
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Test: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`

**Step 1: Write the failing test**

新增断言：
- 显示阶段（计划/执行/汇总）
- 显示轮次
- 显示成员状态 chips（running/completed/failed/timeout）

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.im-routing-panel.test.tsx`  
Expected: FAIL。

**Step 3: Write minimal implementation**

在现有委派卡片基础上新增 group board 数据渲染，接收 orchestrator 事件流。

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/ChatView.im-routing-panel.test.tsx`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src/components/ChatView.tsx apps/runtime/src/types.ts apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx
git commit -m "feat(chat): add group orchestration board for rounds and statuses"
```

### Task 8: 端到端回归与文档

**Files:**
- Modify: `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
- Modify: `README.md`
- Modify: `README.en.md`
- Create: `docs/plans/2026-03-05-group-chat-orchestrator-acceptance.md`

**Step 1: Write the failing test**

新增 E2E 前端回归：
- 拉群后触发一次 run
- 覆盖计划/执行/汇报三阶段渲染
- 覆盖单成员失败降级显示

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.im-feishu-bridge.test.tsx src/components/__tests__/ChatView.im-routing-panel.test.tsx`  
Expected: FAIL。

**Step 3: Write minimal implementation**

补齐事件透传与 UI 文案；更新 README 与验收清单。

**Step 4: Run full verification**

Run: `cd apps/runtime/src-tauri && cargo test -- --nocapture`  
Run: `pnpm --dir apps/runtime exec vitest run`  
Expected: PASS。

**Step 5: Commit**

```bash
git add apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx README.md README.en.md docs/plans/2026-03-05-group-chat-orchestrator-acceptance.md
git commit -m "docs(test): add group orchestrator acceptance and regression coverage"
```

