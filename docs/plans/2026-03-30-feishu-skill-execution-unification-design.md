# Feishu Skill Execution Unification Design

**Date:** 2026-03-30

## Goal

Unify WorkClaw's Feishu skill execution path around an OpenClaw-style skill runtime so natural-language requests keep working, script-heavy leaf skills stop degrading into prompt-only exploratory runs, and the current Feishu PM skills execute faster and more predictably.

This design extends [2026-03-27-openclaw-skill-runtime-alignment-design.md](/d:/code/WorkClaw/docs/plans/2026-03-27-openclaw-skill-runtime-alignment-design.md) with one additional requirement that the earlier alignment work did not fully cover: natural-language skill routing must be able to bridge into deterministic leaf-skill dispatch without requiring the user to type slash commands.

## Problem Statement

The current Feishu PM workflow is slow for two separate reasons:

1. WorkClaw's runtime still treats natural-language skill execution as a prompt-following flow even when the underlying task is a stable script entrypoint.
2. The Feishu skill scripts and shared runtime still do more work than necessary once they are actually invoked.

The captured session at [你能做什么，有什么技能-2026-03-29-2342.md](/d:/code/WorkClaw/temp/你能做什么，有什么技能-2026-03-29-2342.md) shows the agent repeatedly probing PowerShell execution, rewriting helper scripts, and escalating to `task` before it finally returns the expected weekly summary. The runtime log at [pnpm-app.stderr.log](/d:/code/WorkClaw/.codex-logs/pnpm-app.stderr.log) shows the main run reaching `Iteration 40/100` and a child run reaching `Iteration 32/100`, which is far above what a stable leaf-skill execution path should require.

## Root Causes

### 1. Natural-language leaf skill execution still falls back to prompt-following

WorkClaw's prompt assembly already follows the same progressive-disclosure pattern as OpenClaw: list skill descriptions first, then read exactly one matching `SKILL.md`, see [prompt_assembly.rs](/d:/code/WorkClaw/packages/runtime-chat-app/src/prompt_assembly.rs#L64). However, the current `skill` tool still primarily returns structured text instructing the model to follow the skill, see [skill_invoke.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs#L297).

WorkClaw does have deterministic skill dispatch, but it is only auto-triggered for slash-command style user input, see [session_runtime.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs#L102) and [tool_dispatch.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs#L57). Natural-language requests routed to a skill do not yet bridge into that same dispatch lane.

### 2. Workspace skill projection breaks sibling-runtime assumptions

The current Feishu PM scripts assume that sibling skills keep their original directory names. For example, [summarize_pm_employee_work.ps1](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1#L9) looks for a sibling directory named `feishu-pm-runtime`. WorkClaw currently projects skills into the executor workspace using a normalized directory name derived from `skill_id`, see [workspace_skills.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs#L245).

That mismatch explains the observed `feishu-pm-runtime` versus `local-feishu-pm-runtime` confusion in the captured session.

### 3. The current Feishu skills are still authored as prompt-following skills

The Feishu PM hub and weekly summary skills currently declare only `name` and `description`, see [feishu-pm-hub/SKILL.md](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-hub/SKILL.md#L1) and [feishu-pm-weekly-work-summary/SKILL.md](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-weekly-work-summary/SKILL.md#L1). They do not yet declare `command-dispatch`, `command-tool`, or `disable-model-invocation`.

That means even an OpenClaw-style runtime would still treat the current leaf skills as model-driven prompt-following skills unless they are reauthored.

### 4. The script layer does too much work per invocation

The weekly summary script performs three table scans and then filters locally, see [summarize_pm_employee_work.ps1](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-weekly-work-summary/scripts/summarize_pm_employee_work.ps1#L199). The shared base client always uses `--page-all`, see [base_client.ps1](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/base_client.ps1#L10). The task query runtime also does an N+1 detail fetch, see [task_client.ps1](/d:/code/WorkClaw/temp/feishu-pm-skills/feishu-pm-runtime/runtime/scripts/lib/task_client.ps1#L220).

Even after the runtime stops thrashing, those scripts will remain slower than necessary.

## OpenClaw Baseline

OpenClaw is the right architectural baseline, but it does not solve this use case by itself.

- OpenClaw uses progressive-disclosure skill discovery and natural-language model selection, see [system-prompt.ts](/d:/code/WorkClaw/references/openclaw/src/agents/system-prompt.ts#L21) and [workspace.ts](/d:/code/WorkClaw/references/openclaw/src/agents/skills/workspace.ts#L567).
- OpenClaw supports `command-dispatch`, `command-tool`, and `disable-model-invocation`, see [frontmatter.ts](/d:/code/WorkClaw/references/openclaw/src/agents/skills/frontmatter.ts#L208) and [workspace.ts](/d:/code/WorkClaw/references/openclaw/src/agents/skills/workspace.ts#L841).
- OpenClaw's deterministic skill dispatch is currently wired through slash-command handling, see [skill-commands-base.ts](/d:/code/WorkClaw/references/openclaw/src/auto-reply/skill-commands-base.ts#L58) and [get-reply-inline-actions.ts](/d:/code/WorkClaw/references/openclaw/src/auto-reply/reply/get-reply-inline-actions.ts#L181).
- OpenClaw's exec/runtime path on Windows is also more robust, including explicit `.cmd` wrapping and argument escaping, see [exec.ts](/d:/code/WorkClaw/references/openclaw/src/process/exec.ts#L14) and [exec.windows.test.ts](/d:/code/WorkClaw/references/openclaw/src/process/exec.windows.test.ts#L79).

The key conclusion is:

- `Current Feishu skills + OpenClaw runtime` would likely be slightly faster.
- `Reauthored Feishu skills + OpenClaw-aligned runtime + natural-language bridge` is the path to materially faster execution.

## Unified Target Architecture

### 1. Keep the user-facing natural-language entry

Users should continue asking questions such as "看谢涛这周做了什么" without needing slash commands.

### 2. Split skills into hub skills and leaf skills

- Hub skills remain prompt-following and are responsible for:
  - understanding intent
  - asking clarifying questions
  - choosing the correct leaf skill
- Leaf skills become structured execution units and are responsible for:
  - deterministic tool dispatch
  - stable argument contracts
  - script or runtime invocation
  - output normalization

For the Feishu PM family:

- `feishu-pm-hub` stays as the only natural-language entry.
- `feishu-pm-weekly-work-summary`
- `feishu-pm-task-query`
- `feishu-pm-task-dispatch`
- `feishu-pm-daily-monthly-sync`

become deterministic leaf skills.

### 3. Align WorkClaw skill runtime with OpenClaw frontmatter semantics

WorkClaw should fully support:

- `user-invocable`
- `disable-model-invocation`
- `command-dispatch`
- `command-tool`
- `command-arg-mode`
- OpenClaw-style metadata and invocation policy fields

`SKILL.md` becomes the single authoritative runtime contract.

### 4. Add a natural-language-to-leaf dispatch bridge

Once a natural-language request has been routed to a dispatchable leaf skill, WorkClaw should skip the prompt-following execution loop and route directly into the same deterministic dispatch machinery already used by slash commands.

This bridge should:

- preserve natural-language routing and clarification in the hub skill
- preserve policy checks and allowed-tool narrowing
- call deterministic dispatch only when the selected leaf skill declares a valid dispatch contract
- fall back to prompt-following when argument extraction is incomplete or the skill is intentionally non-dispatchable

### 5. Stabilize skill resource resolution

Projected workspace skills must preserve source directory compatibility for sibling skill references and shared runtime assets.

The runtime should either:

- preserve the source directory name as the primary projected directory name, matching OpenClaw's behavior, or
- inject a stable resource root contract that leaf skills use instead of hardcoded sibling names

The preferred model is to do both:

- preserve source directory names where safe
- also add an explicit runtime resource path contract so future skills do not depend on implicit sibling layout

### 6. Normalize leaf-skill execution I/O

Script-oriented leaf skills should follow a strict contract:

- structured result JSON on `stdout`
- diagnostics and progress text on `stderr`
- stable exit codes
- no hidden side-channel file writes needed for normal execution

That contract reduces tool retries and allows WorkClaw to summarize or display results consistently.

### 7. Reduce script-side data-fetch cost

The shared Feishu PM runtime should move toward:

- server-side filtering where available
- bounded pagination instead of unconditional `--page-all`
- batched detail retrieval where possible
- shared helper functions that avoid duplicated scans across leaf skills

## Affected Surfaces

### Runtime contract and projection

- `packages/runtime-skill-core/src/skill_config.rs`
- `packages/runtime-skill-core/tests/skill_config.rs`
- `apps/runtime/src-tauri/src/agent/runtime/runtime_io/types.rs`
- `apps/runtime/src-tauri/src/agent/runtime/runtime_io/workspace_skills.rs`
- `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`

### Skill invocation and dispatch

- `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- `apps/runtime/src-tauri/src/agent/runtime/session_runtime.rs`
- `apps/runtime/src-tauri/src/agent/runtime/tool_dispatch.rs`
- runtime tests covering skill invocation and permission narrowing

### Prompt and catalog behavior

- `packages/runtime-chat-app/src/prompt_assembly.rs`
- prompt assembly tests

### Feishu PM skill assets

- `temp/feishu-pm-skills/feishu-pm-hub/SKILL.md`
- `temp/feishu-pm-skills/feishu-pm-weekly-work-summary/SKILL.md`
- `temp/feishu-pm-skills/feishu-pm-task-query/SKILL.md`
- `temp/feishu-pm-skills/feishu-pm-task-dispatch/SKILL.md`
- `temp/feishu-pm-skills/feishu-pm-daily-monthly-sync/SKILL.md`
- `temp/feishu-pm-skills/feishu-pm-runtime/**`

## Rollout Plan

### Phase 1: Runtime contract alignment

- complete OpenClaw-style frontmatter parsing
- build structured runtime skill entries
- preserve source-compatible skill projection
- keep existing prompt behavior working

### Phase 2: Natural-language bridge

- let natural-language skill routing bridge into deterministic leaf dispatch
- keep slash-command dispatch behavior unchanged
- add explicit fallback to prompt-following when dispatch is unsafe or incomplete

### Phase 3: Feishu PM skill reauthoring

- convert PM leaf skills to structured dispatchable skills
- hide leaf skills from prompt selection where appropriate
- keep `feishu-pm-hub` as the only user-facing natural-language entry

### Phase 4: Shared runtime optimization

- fix runtime root resolution
- reduce table scans
- reduce N+1 task fetches
- normalize stdout/stderr contracts

## Risks

### Runtime behavior risk

Natural-language-to-dispatch bridging changes the execution model for some skill families. The bridge must be explicit and limited to dispatchable leaf skills so descriptive or exploratory skills do not lose flexibility.

### Compatibility risk

Existing local skills may implicitly depend on the current projection layout or prompt-only behavior. The rollout must preserve backward compatibility while the new skill contract is introduced.

### Policy risk

Deterministic dispatch must still obey the same active tool policy and permission gates as ordinary tool calls. The bridge must never become a bypass around allowed-tools narrowing.

### Observability risk

If the runtime changes execution shape but does not improve trace visibility, it will become harder to diagnose regressions. New runtime events and tests should make it obvious when a request used prompt-following versus deterministic dispatch.

## Success Criteria

1. A natural-language Feishu PM request routes through `feishu-pm-hub` and reaches the correct leaf skill without requiring slash commands.
2. Dispatchable PM leaf skills execute through deterministic dispatch instead of repeated exploratory shell attempts.
3. Projected skill resource paths remain stable enough that shared runtime skills resolve correctly.
4. The weekly summary and task-query flows finish in substantially fewer model/tool iterations than the captured slow session.
5. The PM runtime spends less time on full-table scans and N+1 detail fetches.

## Verification Expectations

Implementation should ship with:

- parser tests for the expanded skill frontmatter contract
- workspace skill projection tests covering directory-name stability
- runtime tests proving natural-language routing can bridge into deterministic leaf dispatch
- permission tests proving dispatch still respects allowed-tools narrowing
- prompt assembly tests for skill catalog rules
- skill asset tests or focused runtime tests for the Feishu PM leaf skills

## Release Impact

This is runtime orchestration and skill execution behavior work. It is not a release-metadata or installer change, but it is a high-risk runtime behavior change and should be treated as such during verification.
