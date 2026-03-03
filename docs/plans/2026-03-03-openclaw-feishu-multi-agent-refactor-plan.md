# OpenClaw Feishu Multi-Agent Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Vendor OpenClaw multi-agent routing core into SkillMint, wire it to Feishu-only employee routing, and ship a user-friendly routing wizard without external OpenClaw runtime dependency.

**Architecture:** Keep OpenClaw logic in a vendor boundary under sidecar, expose a stable bridge API for route resolution/simulation, persist routing rules in SQLite via Tauri commands, then integrate Feishu ingress/dispatch and frontend wizard/trace using those commands.

**Tech Stack:** TypeScript (Hono sidecar, node:test), Rust (Tauri, sqlx), React + Vitest, SQLite.

---

### Task 1: Vendor Foundation And Upstream Sync Guardrails

**Files:**
- Create: `apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT`
- Create: `apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md`
- Create: `scripts/sync-openclaw-core.mjs`
- Create: `apps/runtime/sidecar/test/openclaw.vendor-layout.test.ts`
- Modify: `package.json`

**Step 1: Write the failing test**

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

test("openclaw vendor metadata exists", () => {
  assert.equal(existsSync("apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT"), true);
  assert.equal(existsSync("apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md"), true);
  const commit = readFileSync(
    "apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT",
    "utf8",
  ).trim();
  assert.match(commit, /^[0-9a-f]{7,40}$/);
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.vendor-layout.test.ts`  
Expected: FAIL with missing file assertions.

**Step 3: Write minimal implementation**

```js
// scripts/sync-openclaw-core.mjs
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { execSync } from "node:child_process";

const upstreamPath = process.env.OPENCLAW_UPSTREAM_PATH || "temp/openclaw-upstream";
const targetRoot = resolve("apps/runtime/sidecar/vendor/openclaw-core");
mkdirSync(targetRoot, { recursive: true });

const commit = execSync("git rev-parse --short HEAD", {
  cwd: upstreamPath,
  stdio: ["ignore", "pipe", "ignore"],
})
  .toString()
  .trim();
writeFileSync(resolve(targetRoot, "UPSTREAM_COMMIT"), `${commit}\n`, "utf8");

for (const rel of ["src/routing", "src/agents/agent-scope.ts", "src/agents/tool-policy-shared.ts"]) {
  cpSync(resolve(upstreamPath, rel), resolve(targetRoot, rel), { recursive: true });
}

const patches = resolve(targetRoot, "PATCHES.md");
if (!readFileSync(patches, "utf8", { flag: "a+" }).trim()) {
  writeFileSync(patches, "# Local Patches\n\n- none\n", "utf8");
}
```

**Step 4: Run test to verify it passes**

Run: `node scripts/sync-openclaw-core.mjs && pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.vendor-layout.test.ts`  
Expected: PASS.

**Step 5: Commit**

```bash
git add package.json scripts/sync-openclaw-core.mjs apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md apps/runtime/sidecar/test/openclaw.vendor-layout.test.ts
git commit -m "chore(openclaw): add vendor sync and patch metadata"
```

### Task 2: Vendor Routing Wrapper And Parity Test

**Files:**
- Create: `apps/runtime/sidecar/src/openclaw-bridge/route-engine.ts`
- Create: `apps/runtime/sidecar/src/openclaw-bridge/types.ts`
- Create: `apps/runtime/sidecar/test/openclaw.route-engine.test.ts`

**Step 1: Write the failing test**

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { resolveRoute } from "../src/openclaw-bridge/route-engine.js";

test("peer binding wins over account and channel", () => {
  const out = resolveRoute({
    channel: "feishu",
    accountId: "acct-a",
    peer: { kind: "group", id: "chat-1" },
    bindings: [
      { agentId: "channel-agent", match: { channel: "feishu", accountId: "*" } },
      { agentId: "account-agent", match: { channel: "feishu", accountId: "acct-a" } },
      {
        agentId: "peer-agent",
        match: { channel: "feishu", accountId: "acct-a", peer: { kind: "group", id: "chat-1" } },
      },
    ],
    defaultAgentId: "main",
  });
  assert.equal(out.agentId, "peer-agent");
  assert.equal(out.matchedBy, "binding.peer");
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.route-engine.test.ts`  
Expected: FAIL with module not found.

**Step 3: Write minimal implementation**

```ts
// apps/runtime/sidecar/src/openclaw-bridge/route-engine.ts
import { resolveAgentRoute } from "../../vendor/openclaw-core/src/routing/resolve-route.js";
import type { RouteInput, RouteOutput } from "./types.js";

export function resolveRoute(input: RouteInput): RouteOutput {
  const cfg = {
    agents: { list: [{ id: input.defaultAgentId, default: true }] },
    bindings: input.bindings,
  };
  return resolveAgentRoute({
    cfg,
    channel: input.channel,
    accountId: input.accountId,
    peer: input.peer,
    parentPeer: input.parentPeer,
    guildId: input.guildId,
    teamId: input.teamId,
    memberRoleIds: input.memberRoleIds,
  });
}
```

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.route-engine.test.ts`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/openclaw-bridge/types.ts apps/runtime/sidecar/src/openclaw-bridge/route-engine.ts apps/runtime/sidecar/test/openclaw.route-engine.test.ts
git commit -m "feat(sidecar): add vendored openclaw route engine wrapper"
```

### Task 3: Expose Sidecar Route Resolve/Simulate APIs

**Files:**
- Modify: `apps/runtime/sidecar/src/index.ts`
- Create: `apps/runtime/sidecar/test/openclaw.route-api.test.ts`

**Step 1: Write the failing test**

```ts
import test from "node:test";
import assert from "node:assert/strict";
import app from "../src/index.js";

test("route resolve endpoint returns matched route", async () => {
  const req = new Request("http://localhost/api/openclaw/resolve-route", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      channel: "feishu",
      account_id: "acct-a",
      peer: { kind: "group", id: "chat-1" },
      default_agent_id: "main",
      bindings: [{ agentId: "main", match: { channel: "feishu", accountId: "*" } }],
    }),
  });
  const res = await app.fetch(req);
  const json = await res.json();
  assert.equal(Boolean(json.output), true);
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.route-api.test.ts`  
Expected: FAIL with 404 route not found.

**Step 3: Write minimal implementation**

```ts
app.post("/api/openclaw/resolve-route", async (c) => {
  try {
    const body = await c.req.json();
    const result = resolveRoute({
      channel: body.channel,
      accountId: body.account_id,
      peer: body.peer ?? null,
      parentPeer: body.parent_peer ?? null,
      guildId: body.guild_id ?? undefined,
      teamId: body.team_id ?? undefined,
      memberRoleIds: body.member_role_ids ?? [],
      bindings: body.bindings ?? [],
      defaultAgentId: body.default_agent_id || "main",
    });
    return c.json({ output: JSON.stringify(result) } as ApiResponse);
  } catch (e: any) {
    return c.json({ error: e.message } as ApiResponse, 400);
  }
});
```

**Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime/sidecar exec tsx --test test/openclaw.route-api.test.ts`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/src/index.ts apps/runtime/sidecar/test/openclaw.route-api.test.ts
git commit -m "feat(sidecar): expose openclaw route resolve API"
```

### Task 4: Add Routing Binding Persistence And Commands (Rust)

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Create: `apps/runtime/src-tauri/src/commands/im_routing.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/src/lib.rs`
- Modify: `apps/runtime/src-tauri/tests/helpers/mod.rs`
- Create: `apps/runtime/src-tauri/tests/test_im_routing_bindings.rs`

**Step 1: Write the failing test**

```rust
mod helpers;

use runtime_lib::commands::im_routing::{
    list_im_routing_bindings_with_pool, upsert_im_routing_binding_with_pool, UpsertImRoutingBindingInput,
};

#[tokio::test]
async fn upsert_and_list_routing_bindings() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    upsert_im_routing_binding_with_pool(&pool, UpsertImRoutingBindingInput {
        id: None,
        agent_id: "main".to_string(),
        channel: "feishu".to_string(),
        account_id: "*".to_string(),
        peer_kind: "".to_string(),
        peer_id: "".to_string(),
        guild_id: "".to_string(),
        team_id: "".to_string(),
        role_ids: vec![],
        priority: 100,
        enabled: true,
    }).await.expect("upsert");

    let rows = list_im_routing_bindings_with_pool(&pool).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].channel, "feishu");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_routing_bindings -- --nocapture`  
Expected: FAIL with missing table/command types.

**Step 3: Write minimal implementation**

```rust
// db.rs
sqlx::query(
  "CREATE TABLE IF NOT EXISTS im_routing_bindings (
      id TEXT PRIMARY KEY,
      agent_id TEXT NOT NULL,
      channel TEXT NOT NULL,
      account_id TEXT NOT NULL DEFAULT '',
      peer_kind TEXT NOT NULL DEFAULT '',
      peer_id TEXT NOT NULL DEFAULT '',
      guild_id TEXT NOT NULL DEFAULT '',
      team_id TEXT NOT NULL DEFAULT '',
      role_ids_json TEXT NOT NULL DEFAULT '[]',
      priority INTEGER NOT NULL DEFAULT 100,
      enabled INTEGER NOT NULL DEFAULT 1,
      created_at TEXT NOT NULL,
      updated_at TEXT NOT NULL
  )"
).execute(&pool).await?;
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_routing_bindings -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/im_routing.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/src/lib.rs apps/runtime/src-tauri/tests/helpers/mod.rs apps/runtime/src-tauri/tests/test_im_routing_bindings.rs
git commit -m "feat(im): add routing bindings persistence and tauri commands"
```

### Task 5: Extend Employee Model For OpenClaw Mapping

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/employees/EmployeeHubView.tsx`
- Modify: `apps/runtime/src-tauri/tests/test_im_employee_agents.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn employee_persists_openclaw_agent_mapping() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let id = upsert_agent_employee_with_pool(&pool, UpsertAgentEmployeeInput {
        id: None,
        name: "主员工".to_string(),
        role_id: "main".to_string(),
        persona: "".to_string(),
        feishu_open_id: "".to_string(),
        feishu_app_id: "".to_string(),
        feishu_app_secret: "".to_string(),
        primary_skill_id: "".to_string(),
        default_work_dir: "".to_string(),
        enabled: true,
        is_default: true,
        skill_ids: vec![],
        openclaw_agent_id: "main".to_string(),
        routing_priority: 100,
        enabled_scopes: vec!["feishu".to_string()],
    }).await.expect("upsert");
    let list = list_agent_employees_with_pool(&pool).await.expect("list");
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].openclaw_agent_id, "main");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_employee_agents -- --nocapture`  
Expected: FAIL with struct field mismatch.

**Step 3: Write minimal implementation**

```rust
// employee_agents.rs (struct fields)
pub openclaw_agent_id: String,
pub routing_priority: i64,
pub enabled_scopes: Vec<String>,
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_employee_agents -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src/types.ts apps/runtime/src/components/employees/EmployeeHubView.tsx apps/runtime/src-tauri/tests/test_im_employee_agents.rs
git commit -m "feat(employee): add openclaw agent mapping fields"
```

### Task 6: Add Route Resolution Command And Sidecar Bridge Call

**Files:**
- Modify: `apps/runtime/src-tauri/src/commands/openclaw_gateway.rs`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`
- Modify: `apps/runtime/src-tauri/tests/test_openclaw_gateway.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn resolve_route_prefers_peer_binding() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    // seed two bindings: account fallback + peer specific
    // call resolve_openclaw_route_with_pool(...)
    // assert matched_by == "binding.peer"
    assert!(false, "placeholder");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_gateway -- --nocapture`  
Expected: FAIL.

**Step 3: Write minimal implementation**

```rust
pub async fn resolve_openclaw_route_with_pool(
    pool: &SqlitePool,
    event: &ImEvent,
) -> Result<serde_json::Value, String> {
    let bindings = list_im_routing_bindings_with_pool(pool).await?;
    let body = serde_json::json!({
        "channel": "feishu",
        "account_id": event.tenant_id.clone().unwrap_or_default(),
        "peer": { "kind": "group", "id": event.thread_id },
        "default_agent_id": "main",
        "bindings": bindings_to_openclaw_payload(bindings),
    });
    call_sidecar_json(resolve_feishu_sidecar_base_url(pool, None).await, "/api/openclaw/resolve-route", &body).await
}
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_gateway -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/openclaw_gateway.rs apps/runtime/src-tauri/src/commands/feishu_gateway.rs apps/runtime/src-tauri/tests/test_openclaw_gateway.rs
git commit -m "feat(openclaw): resolve route through vendored sidecar engine"
```

### Task 7: Route-Key-Based Session Reuse And Dispatch

**Files:**
- Modify: `apps/runtime/src-tauri/src/db.rs`
- Modify: `apps/runtime/src-tauri/src/commands/employee_agents.rs`
- Modify: `apps/runtime/src-tauri/tests/test_im_recent_threads.rs`
- Create: `apps/runtime/src-tauri/tests/test_im_route_session_mapping.rs`

**Step 1: Write the failing test**

```rust
#[tokio::test]
async fn same_route_session_key_reuses_existing_session() {
    // first event creates session, second event with same route key reuses it
    assert!(false, "placeholder");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_route_session_mapping -- --nocapture`  
Expected: FAIL.

**Step 3: Write minimal implementation**

```rust
// db.rs migration
let _ = sqlx::query("ALTER TABLE im_thread_sessions ADD COLUMN route_session_key TEXT NOT NULL DEFAULT ''")
    .execute(&pool).await;
let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_im_thread_sessions_route_key ON im_thread_sessions(route_session_key)")
    .execute(&pool).await;
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_im_route_session_mapping -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/db.rs apps/runtime/src-tauri/src/commands/employee_agents.rs apps/runtime/src-tauri/tests/test_im_recent_threads.rs apps/runtime/src-tauri/tests/test_im_route_session_mapping.rs
git commit -m "feat(im): map thread sessions by openclaw route session key"
```

### Task 8: Feishu Routing Wizard Backend And Frontend Integration

**Files:**
- Create: `apps/runtime/src/components/employees/FeishuRoutingWizard.tsx`
- Modify: `apps/runtime/src/components/SettingsView.tsx`
- Modify: `apps/runtime/src/types.ts`
- Create: `apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx`

**Step 1: Write the failing test**

```tsx
it("saves routing rule from wizard and can run simulation", async () => {
  // render SettingsView -> Feishu tab -> open wizard
  // fill rule form -> click save
  // assert invoke("upsert_im_routing_binding", ...) called
  // click simulate -> assert invoke("simulate_im_route", ...) called
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test SettingsView.feishu-routing-wizard.test.tsx`  
Expected: FAIL.

**Step 3: Write minimal implementation**

```tsx
<FeishuRoutingWizard
  bindings={routingBindings}
  onSaveRule={(input) => invoke("upsert_im_routing_binding", { input })}
  onDeleteRule={(id) => invoke("delete_im_routing_binding", { id })}
  onSimulate={(payload) => invoke("simulate_im_route", { payload })}
/>
```

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test SettingsView.feishu-routing-wizard.test.tsx`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/components/employees/FeishuRoutingWizard.tsx apps/runtime/src/components/SettingsView.tsx apps/runtime/src/types.ts apps/runtime/src/components/__tests__/SettingsView.feishu-routing-wizard.test.tsx
git commit -m "feat(ui): add feishu routing wizard for openclaw-style bindings"
```

### Task 9: Route Decision Trace Events In Chat Panel

**Files:**
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/components/ChatView.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx`
- Modify: `apps/runtime/src-tauri/src/commands/feishu_gateway.rs`

**Step 1: Write the failing test**

```tsx
it("renders route decision card with matched_by and session_key", async () => {
  // emit "im-route-decision" event
  // assert card contains matched_by + session_key + agent_id
});
```

**Step 2: Run test to verify it fails**

Run: `pnpm --filter runtime test ChatView.im-routing-panel.test.tsx`  
Expected: FAIL.

**Step 3: Write minimal implementation**

```ts
export interface ImRouteDecisionEvent {
  thread_id: string;
  agent_id: string;
  session_key: string;
  matched_by: string;
}
```

**Step 4: Run test to verify it passes**

Run: `pnpm --filter runtime test ChatView.im-routing-panel.test.tsx`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src/types.ts apps/runtime/src/components/ChatView.tsx apps/runtime/src/components/__tests__/ChatView.im-routing-panel.test.tsx apps/runtime/src-tauri/src/commands/feishu_gateway.rs
git commit -m "feat(trace): show openclaw route decisions in chat timeline"
```

### Task 10: Upgrade Playbook, Regression Suite, And Documentation

**Files:**
- Create: `apps/runtime/sidecar/vendor/openclaw-core/README.md`
- Create: `apps/runtime/src-tauri/tests/test_openclaw_route_regression.rs`
- Modify: `README.zh-CN.md`
- Modify: `README.md`

**Step 1: Write the failing test**

```rust
#[test]
fn route_regression_vectors_match_expected_priority() {
    // table-driven vectors: peer/account/channel/default
    // assert matched_by and agent_id
    assert!(false, "placeholder");
}
```

**Step 2: Run test to verify it fails**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_route_regression -- --nocapture`  
Expected: FAIL.

**Step 3: Write minimal implementation**

```md
<!-- vendor/openclaw-core/README.md -->
## Upstream Sync
1. Set `OPENCLAW_UPSTREAM_PATH`
2. Run `node scripts/sync-openclaw-core.mjs`
3. Update `PATCHES.md`
4. Run regression tests
```

**Step 4: Run test to verify it passes**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_route_regression -- --nocapture`  
Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/sidecar/vendor/openclaw-core/README.md apps/runtime/src-tauri/tests/test_openclaw_route_regression.rs README.zh-CN.md README.md
git commit -m "docs(openclaw): add upgrade playbook and regression vectors"
```

### Task 11: Full Verification Before Completion

**Files:**
- Modify: `docs/plans/2026-03-03-openclaw-feishu-multi-agent-refactor-design.md` (only if implementation drifted)

**Step 1: Run sidecar tests**

Run: `pnpm --dir apps/runtime/sidecar test`  
Expected: PASS.

**Step 2: Run Rust IM/OpenClaw test subset**

Run: `cd apps/runtime/src-tauri && cargo test --test test_openclaw_gateway --test test_im_routing_bindings --test test_im_route_session_mapping --test test_feishu_gateway -- --nocapture`  
Expected: PASS.

**Step 3: Run frontend tests**

Run: `pnpm --filter runtime test`  
Expected: PASS.

**Step 4: Run frontend build**

Run: `pnpm --filter runtime build`  
Expected: PASS.

**Step 5: Final commit (if verification/doc updates exist)**

```bash
git add docs/plans/2026-03-03-openclaw-feishu-multi-agent-refactor-design.md
git commit -m "chore: finalize openclaw feishu multi-agent refactor verification"
```
