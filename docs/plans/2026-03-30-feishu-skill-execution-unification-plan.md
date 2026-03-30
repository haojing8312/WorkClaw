# Feishu Skill Execution Unification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild WorkClaw's Feishu skill execution path so natural-language requests route through a hub skill but dispatchable leaf skills execute deterministically through an OpenClaw-aligned runtime.

**Architecture:** Extend the existing OpenClaw skill-runtime alignment work with a natural-language dispatch bridge and Feishu skill reauthoring. First align frontmatter parsing, projection, and dispatch contracts with OpenClaw semantics. Then teach WorkClaw to bridge natural-language hub routing into deterministic leaf dispatch. Finally convert the Feishu PM leaf skills and shared runtime to the new execution contract and reduce script-side query cost.

**Tech Stack:** Rust, Tauri runtime, `runtime-skill-core`, `runtime-chat-app`, projected workspace skills, PowerShell, embedded `lark-cli`, SQLite-backed runtime state, WorkClaw integration tests.

---

### Task 1: Lock the OpenClaw-aligned skill contract

**Files:**
- Modify: `packages/runtime-skill-core/src/skill_config.rs`
- Test: `packages/runtime-skill-core/tests/skill_config.rs`
- Reference: `references/openclaw/src/agents/skills/frontmatter.ts`
- Reference: `references/openclaw/src/agents/skills/types.ts`

**Step 1: Write the failing parser tests**

Add frontmatter fixtures covering:

- `user-invocable`
- `disable-model-invocation`
- `command-dispatch`
- `command-tool`
- `command-arg-mode`
- OpenClaw-style metadata blocks used by runtime decisions

**Step 2: Run the parser tests and verify they fail**

Run: `cargo test -p runtime-skill-core --test skill_config`

Expected: the new assertions fail because the fields are not fully parsed yet.

**Step 3: Extend `SkillConfig` with the missing parsed fields**

Add the minimal Rust structs and parser support needed for:

- invocation policy
- command dispatch
- metadata used by runtime projection

Keep existing fields backward-compatible.

**Step 4: Re-run the parser tests**

Run: `cargo test -p runtime-skill-core --test skill_config`

Expected: PASS.

**Step 5: Commit**

```bash
git add packages/runtime-skill-core/src/skill_config.rs packages/runtime-skill-core/tests/skill_config.rs
git commit -m "feat: align skill frontmatter contract with openclaw"
```

### Task 2: Preserve source-compatible skill projection

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/types.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs`
- Reference: `references/openclaw/src/agents/skills/workspace.ts`

**Step 1: Write failing projection tests**

Add tests that prove:

- projected skill directories preserve source-compatible names
- sibling runtime skills remain addressable after projection
- prompt-visible skill paths still resolve correctly

**Step 2: Run the targeted projection tests**

Run: `cargo test -p runtime_lib workspace_skill_projection`

Expected: FAIL because projection currently follows `skill_id`-derived naming.

**Step 3: Implement source-compatible projection behavior**

Update runtime skill entry construction and projection so the executor workspace keeps a stable directory name contract compatible with sibling skill references.

**Step 4: Keep prompt and invoke-name behavior stable**

Do not change public prompt catalog selection semantics while fixing projection layout.

**Step 5: Re-run the targeted projection tests**

Run: `cargo test -p runtime_lib workspace_skill_projection`

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/types.rs
git commit -m "feat: preserve stable workspace skill projection paths"
```

### Task 3: Expose structured leaf-skill resolution in the runtime

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- Test: `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- Test: `apps/runtime/src-tauri/tests/test_skill_permission_narrowing.rs`

**Step 1: Write failing tests for dispatchable versus prompt-following skills**

Cover:

- prompt-following hub skill resolution
- dispatchable leaf skill resolution
- `disable-model-invocation` handling
- policy denial when dispatch targets a blocked tool

**Step 2: Run the targeted tests and verify they fail**

Run: `cargo test -p runtime_lib skill_invoke`

Expected: FAIL because `skill` still mostly renders instructions instead of exposing structured resolution outcomes.

**Step 3: Refactor the `skill` tool to return structured resolution state**

Preserve the current prompt-following behavior for hub skills, but make dispatch metadata first-class so later runtime callers can bridge directly into deterministic dispatch.

**Step 4: Re-run the targeted tests**

Run: `cargo test -p runtime_lib skill_invoke`

Expected: PASS.

**Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs apps/runtime/src-tauri/tests/test_skill_permission_narrowing.rs
git commit -m "feat: expose structured skill resolution for runtime dispatch"
```

### Task 4: Add the natural-language-to-leaf dispatch bridge

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Test: add or extend `apps/runtime/src-tauri/tests/` coverage for skill routing
- Reference: `references/openclaw/src/auto-reply/reply/get-reply-inline-actions.ts`

**Step 1: Write a failing runtime test for natural-language Feishu hub routing**

Model the path:

- user sends a natural-language PM request
- hub skill is selected
- leaf skill is resolved
- deterministic dispatch runs without requiring `/command`

**Step 2: Run the targeted runtime test**

Run: `cargo test -p runtime_lib natural_language_skill_dispatch`

Expected: FAIL because only slash-command input currently reaches automatic dispatch.

**Step 3: Implement the bridge**

Add a runtime path that:

- keeps the hub skill model-driven
- detects when the chosen leaf skill is dispatchable
- forwards the resolved args through existing deterministic dispatch machinery
- falls back to prompt-following when dispatch is incomplete or unsafe

**Step 4: Preserve slash-command behavior**

Ensure `/skill` and existing user-invocable dispatch paths keep working without behavior regressions.

**Step 5: Re-run the targeted runtime test**

Run: `cargo test -p runtime_lib natural_language_skill_dispatch`

Expected: PASS.

**Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs
git commit -m "feat: bridge natural language skill routing into dispatch"
```

### Task 5: Tighten prompt and catalog behavior for hub plus leaf skills

**Files:**
- Modify: `packages/runtime-chat-app/src/prompt_assembly.rs`
- Test: `packages/runtime-chat-app/tests/prompt_assembly.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs`

**Step 1: Write failing prompt/catalog tests**

Cover:

- only hub skills appear in the prompt when leaf skills set `disable-model-invocation`
- prompt instructions still tell the model to read exactly one matching `SKILL.md`
- command specs are still generated for user-invocable leaf skills

**Step 2: Run the prompt tests**

Run: `cargo test -p runtime-chat-app prompt_assembly`

Expected: FAIL until prompt/catalog behavior reflects the new hub-plus-leaf split.

**Step 3: Implement the minimal prompt and catalog changes**

Keep WorkClaw's progressive-disclosure model while allowing hidden leaf skills to remain dispatchable.

**Step 4: Re-run the prompt tests**

Run: `cargo test -p runtime-chat-app prompt_assembly`

Expected: PASS.

**Step 5: Commit**

```bash
git add packages/runtime-chat-app/src/prompt_assembly.rs packages/runtime-chat-app/tests/prompt_assembly.rs apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs
git commit -m "feat: separate prompt-visible hub skills from dispatchable leaves"
```

### Task 6: Reauthor the Feishu PM skills into hub plus leaf form

**Files:**
- Modify: `temp/feishu-pm-skills/feishu-pm-hub/SKILL.md`
- Modify: `temp/feishu-pm-skills/feishu-pm-weekly-work-summary/SKILL.md`
- Modify: `temp/feishu-pm-skills/feishu-pm-task-query/SKILL.md`
- Modify: `temp/feishu-pm-skills/feishu-pm-task-dispatch/SKILL.md`
- Modify: `temp/feishu-pm-skills/feishu-pm-daily-monthly-sync/SKILL.md`
- Test: `pnpm test:builtin-skills` if these assets are wired into the runtime test lane

**Step 1: Write the failing skill contract expectations**

Document or codify tests for:

- hub remains prompt-following
- leaf skills declare structured dispatch metadata
- leaf skills can be hidden from prompt selection when needed

**Step 2: Update the hub skill**

Keep `feishu-pm-hub` as the natural-language router only.

**Step 3: Update each leaf skill**

Give each leaf skill:

- a stable command contract
- dispatch metadata
- precise argument conventions
- a clear stdout/stderr execution contract

**Step 4: Run the relevant skill validation lane**

Run: `pnpm test:builtin-skills`

Expected: PASS or a focused explanation of any uncovered local-skill gap.

**Step 5: Commit**

```bash
git add temp/feishu-pm-skills/feishu-pm-hub/SKILL.md temp/feishu-pm-skills/feishu-pm-weekly-work-summary/SKILL.md temp/feishu-pm-skills/feishu-pm-task-query/SKILL.md temp/feishu-pm-skills/feishu-pm-task-dispatch/SKILL.md temp/feishu-pm-skills/feishu-pm-daily-monthly-sync/SKILL.md
git commit -m "feat: convert feishu pm skills to hub and leaf execution contracts"
```

### Task 7: Fix shared runtime root resolution and output contract

**Files:**
- Modify: `temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1`
- Modify: `temp/feishu-pm-skills/feishu-pm-task-query/scripts/*` as needed
- Modify: `temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/config.ps1`
- Modify: `temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/common.ps1`
- Add or modify focused script-side helper tests if available

**Step 1: Make runtime root resolution explicit**

Replace hardcoded sibling-directory assumptions with a stable runtime resource contract, while keeping a fallback for older projected layouts during migration.

**Step 2: Normalize output behavior**

Ensure:

- result JSON goes to stdout
- diagnostics go to stderr
- exit codes are reliable

**Step 3: Run a direct script smoke test**

Run a focused PowerShell invocation for the weekly summary skill using known-good local config.

Expected: JSON on stdout and diagnostics on stderr.

**Step 4: Commit**

```bash
git add temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1 temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/config.ps1 temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/common.ps1
git commit -m "fix: stabilize feishu skill runtime path and output contracts"
```

### Task 8: Remove the biggest script-side query bottlenecks

**Files:**
- Modify: `temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/base_client.ps1`
- Modify: `temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/task_client.ps1`
- Modify: `temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1`
- Modify any affected task-query scripts

**Step 1: Write down the failing performance assumptions**

Capture the current costly behaviors:

- unconditional `--page-all`
- local filtering after full scans
- N+1 task detail fetches

**Step 2: Implement the smallest safe fetch optimizations**

Prefer:

- server-side filtering where supported
- bounded pagination
- fewer redundant scans
- fewer per-task detail requests

Do not rewrite the whole PM runtime if a smaller change solves the main problem.

**Step 3: Run direct script timing checks**

Measure the weekly summary and task-query flows before and after the changes using the same local environment and input window.

Expected: materially lower script runtime than the current baseline.

**Step 4: Commit**

```bash
git add temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/base_client.ps1 temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/task_client.ps1 temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1
git commit -m "perf: reduce feishu pm runtime scan and n-plus-one cost"
```

### Task 9: Run end-to-end verification on the unified flow

**Files:**
- Verify runtime and tests touched above
- Optionally add focused regression tests near the touched runtime modules

**Step 1: Run Rust fast-path verification**

Run: `pnpm test:rust-fast`

Expected: PASS.

**Step 2: Run side effects or asset verification that applies to the touched surface**

Run: `pnpm test:builtin-skills`

Expected: PASS, or a clear note about any lane that does not currently cover local Feishu skill assets.

**Step 3: Run a manual natural-language smoke check**

Use the desktop/runtime dev flow and validate:

- user sends a natural-language PM request
- hub routes correctly
- leaf dispatch executes without repeated helper script thrash
- result returns in substantially fewer iterations than the captured slow run

**Step 4: Record any remaining risk**

If any area remains unverified, note whether it is:

- a missing automated test lane
- an environment-specific Feishu credential dependency
- a local-only skill asset coverage gap

**Step 5: Commit the final verification-adjacent test updates**

```bash
git add apps/runtime/src-tauri/tests packages/runtime-chat-app/tests
git commit -m "test: cover unified feishu skill execution flow"
```
