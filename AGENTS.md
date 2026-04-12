# AGENTS.md instructions for E:/code/yzpd/workclaw

## Project Overview
- `apps/runtime/` contains the React desktop app shell, frontend flows, and UI tests.
- `apps/runtime/src-tauri/` contains the Tauri backend, desktop integrations, and Rust integration tests.
- `apps/runtime/sidecar/` contains the sidecar runtime, adapters, browser automation bridge, and sidecar tests.
- `packages/*` contains shared Rust crates for routing, policy, models, executor, skill packaging, and runtime support.
- Root `package.json` commands are the source of truth for local verification and release-sensitive checks.

## Repo-Local Workflow Skills
- `$workclaw-implementation-strategy`: Use before editing runtime behavior, routing, provider integration, tool permissions, sidecar bridge behavior, or vendor sync boundaries.
- `$workclaw-change-verification`: Use when changes affect code, tests, builtin skill assets, or build/test behavior before claiming the work is complete.
- `$workclaw-release-readiness`: Use when changes affect versioning, release documentation, installer branding, packaging outputs, or vendor release lanes before deciding a branch is safe to ship.
- `$workclaw-release-prep`: Use before publishing to recommend the next version and draft bilingual Chinese + English release notes for confirmation.
- `$workclaw-release-publish`: Use only after version and release notes are confirmed, to update release metadata, push the release tag, and generate local desktop artifacts.

## Repo Hygiene Governance

- Treat orphan files, dead code, stale docs, duplicate implementations, and temporary artifacts as a maintenance surface, not a one-off cleanup task.
- Prefer repo hygiene review before deletion. Do not remove suspicious files or code only because they appear unused in one static signal.
- Use `pnpm review:repo-hygiene` for non-blocking repo hygiene reporting when the task is cleanup-focused or when a large feature leaves likely follow-up debris.
- Use focused repo hygiene subcommands when one narrow signal is enough:
  - `pnpm review:repo-hygiene:deadcode`
  - `pnpm review:repo-hygiene:artifacts`
  - `pnpm review:repo-hygiene:drift`
  - `pnpm review:repo-hygiene:dup`
  - `pnpm review:repo-hygiene:loc`
  - `pnpm review:repo-hygiene:cycles`
- Treat duplicate implementations, oversized files, and import cycles as review-first governance signals. They should trigger triage and split plans, not blind deletion or mechanical rewrites.
- Use `workclaw-repo-hygiene-review` to classify candidates and recommend the smallest safe cleanup batch before destructive edits.
- Use `workclaw-cleanup-execution` only after review selected a cleanup batch and its reviewed action per file.
- Cleanup changes still require `workclaw-change-verification` when code, tests, docs, or skill files change.
- Treat generated, runtime-owned, dynamically discovered, or config-driven files as high-risk cleanup surfaces unless a rule explicitly marks them safe.

## Current Project Stage
- WorkClaw is currently an early-stage open source project with a single primary maintainer.
- Default development may happen directly on `main` when that is the most practical path.
- Repo-local skills are lightweight self-check and workflow guidance tools, not mandatory PR approval gates.
- PR-based review, automated merge gates, and stricter branch policies are optional future upgrades, not the default workflow today.

## Mandatory Skill Usage
- Use `$workclaw-implementation-strategy` before changing runtime behavior, routing, provider integration, tool permissions, sidecar protocols, IM orchestration behavior, or vendor sync boundaries.
- Use `$workclaw-change-verification` when changes affect runtime code, tests, examples, builtin skills, or build/test behavior. Do not claim completion until the relevant checks have actually run.
- Use `$workclaw-release-readiness` when changes affect versions, release docs, installer branding, packaging, or vendor release lanes.

These skills should be treated as lightweight guardrails for the maintainer's own workflow. They do not imply that every change must go through a PR or a separate human approval step.

## Build And Test Commands
- Runtime dev: `pnpm app`
- Desktop build: `pnpm build:runtime`
- Sidecar tests: `pnpm test:sidecar`
- Rust fast path: `pnpm test:rust-fast`
- Runtime E2E: `pnpm test:e2e:runtime`
- Builtin skills: `pnpm test:builtin-skills`
- Real agent evals: `pnpm eval:agent-real --scenario <id>`

## Release-Sensitive Commands
- Version checks: `pnpm release:check-version`
- Release tests: `pnpm test:release`
- Installer checks: `pnpm test:installer`
- Release docs: `pnpm test:release-docs`
- Vendor lane checks: `pnpm test:openclaw-vendor-lane`
- Packaging sanity: `pnpm build:runtime`

## Compatibility And Safety Rules
- Preserve existing user-visible runtime behavior unless the change is intentional and called out explicitly.
- Treat packaging, installer, release docs, and vendor sync changes as release-sensitive, not ordinary code edits.
- Prefer the smallest command set that proves the touched area is verified, but never skip a required check for the changed surface.
- Verification claims must cite the commands actually run and whether any areas remain unverified.
- For SQLite-backed runtime data, any new query dependency on a column or table shape must ship with a backward-compatible migration or a legacy-schema fallback in the query path.
- When changing session list, session search, IM bindings, or other startup-critical SQLite reads, add at least one regression test that uses a legacy schema and proves old databases still load.

## Rust Runtime Guidance
- For work under `apps/runtime/src-tauri/`, prefer the closer local guidance in `apps/runtime/src-tauri/AGENTS.md`.
- Rust runtime file budgets use governance triggers rather than hard bans:
  - `<= 500` target
  - `501-800` warning for new business logic
  - `801+` requires a short split plan before feature work
- Keep the root file short; Rust-specific module placement and layering rules belong in the local Tauri guidance file.
- Current Rust-side reference templates for giant command-file governance are:
  - `apps/runtime/src-tauri/src/commands/employee_agents.rs`
  - `apps/runtime/src-tauri/src/commands/openclaw_plugins.rs`
- Reuse these before inventing a new split pattern for the next giant Rust command surface.

## Frontend Runtime Guidance
- For work under `apps/runtime/src/`, prefer the closer local guidance in `apps/runtime/src/AGENTS.md`.
- Frontend runtime file budgets use governance triggers rather than hard bans:
  - `<= 300` target
  - `301-500` warning for new page state, Tauri I/O, or major render branches
  - `501+` requires a short split plan before feature work
- Keep the root file short; frontend-specific module placement and layering rules belong in the local runtime guidance file.
- Use the current `App.tsx -> scenes/* -> components/*` direction as the baseline split pattern for shrinking large frontend runtime files.

## Skill Priority And Coordination
- Treat repo-local `workclaw-*` skills as the project workflow layer. They decide which WorkClaw-specific path, commands, and output contract apply.
- Treat `superpowers` skills as the general method layer. They guide how to design, debug, verify, review, and execute work once the WorkClaw-specific path is known.
- When both apply, use the repo-local `workclaw-*` skill first to choose the right repo workflow, then apply the relevant `superpowers` skill to execute that workflow well.
- `workclaw-change-verification` defines which WorkClaw verification commands are required; `verification-before-completion` still applies before claiming success.
- `workclaw-implementation-strategy` handles WorkClaw-specific risk and boundary analysis; `brainstorming` still applies for broader feature or behavior design work.
- `workclaw-release-readiness` handles ship-readiness for WorkClaw release-sensitive changes; branch-finish or review skills can still apply afterward.
- In the current project stage, prefer simple direct execution over introducing PR ceremony unless the change is risky enough to benefit from that extra process.

## Skills
A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.

### Available skills
- Repo-local skills above are primary for WorkClaw. The global skills below are supplementary method skills.
- workclaw-implementation-strategy: Review risky runtime, routing, provider, permission, sidecar, or vendor-boundary changes before editing code. (file: .agents/skills/workclaw-implementation-strategy/SKILL.md)
- workclaw-change-verification: Choose and run the correct verification commands when WorkClaw changes affect runtime code, tests, skill assets, or build/test behavior. (file: .agents/skills/workclaw-change-verification/SKILL.md)
- workclaw-release-readiness: Review versioning, installer, release-doc, packaging, and vendor-lane changes before deciding a branch is safe to ship. (file: .agents/skills/workclaw-release-readiness/SKILL.md)
- workclaw-release-prep: Recommend the next WorkClaw release version and draft confirmation-ready bilingual release notes before publishing. (file: .agents/skills/workclaw-release-prep/SKILL.md)
- workclaw-release-publish: Publish a confirmed WorkClaw release by updating metadata, pushing the tag, and generating local Windows installers. (file: .agents/skills/workclaw-release-publish/SKILL.md)
- skill-creator: Create or update reusable skills. (file: C:/Users/36443/.codex/skills/.system/skill-creator/SKILL.md)
- skill-installer: Install curated or repo-based skills. (file: C:/Users/36443/.codex/skills/.system/skill-installer/SKILL.md)
- brainstorming: Design or clarify work before implementation. (file: D:/worksoftdata/.codex/superpowers/skills/brainstorming/SKILL.md)
- dispatching-parallel-agents: Split independent work across parallel agents. (file: D:/worksoftdata/.codex/superpowers/skills/dispatching-parallel-agents/SKILL.md)
- executing-plans: Execute a written implementation plan in batches. (file: D:/worksoftdata/.codex/superpowers/skills/executing-plans/SKILL.md)
- finishing-a-development-branch: Wrap up completed work for merge, PR, or cleanup. (file: D:/worksoftdata/.codex/superpowers/skills/finishing-a-development-branch/SKILL.md)
- receiving-code-review: Evaluate incoming review feedback critically before changing code. (file: D:/worksoftdata/.codex/superpowers/skills/receiving-code-review/SKILL.md)
- requesting-code-review: Ask for review before merging important work. (file: D:/worksoftdata/.codex/superpowers/skills/requesting-code-review/SKILL.md)
- subagent-driven-development: Execute plan tasks with subagents in the current session. (file: D:/worksoftdata/.codex/superpowers/skills/subagent-driven-development/SKILL.md)
- systematic-debugging: Debug failures or unexpected behavior methodically. (file: D:/worksoftdata/.codex/superpowers/skills/systematic-debugging/SKILL.md)
- test-driven-development: Use TDD before implementing features or bugfixes. (file: D:/worksoftdata/.codex/superpowers/skills/test-driven-development/SKILL.md)
- using-git-worktrees: Create an isolated worktree before risky or planned work. (file: D:/worksoftdata/.codex/superpowers/skills/using-git-worktrees/SKILL.md)
- using-superpowers: Load and apply skills correctly at conversation start. (file: D:/worksoftdata/.codex/superpowers/skills/using-superpowers/SKILL.md)
- verification-before-completion: Require fresh verification evidence before success claims. (file: D:/worksoftdata/.codex/superpowers/skills/verification-before-completion/SKILL.md)
- writing-plans: Write a detailed implementation plan before code changes. (file: D:/worksoftdata/.codex/superpowers/skills/writing-plans/SKILL.md)
- writing-skills: Create or refine skills with clear trigger wording. (file: D:/worksoftdata/.codex/superpowers/skills/writing-skills/SKILL.md)

### How to use skills
- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the skill directory listed above first, and only consider other paths if needed.
  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.

## Process Safety Rules
- Never kill all processes by image name (for example: `taskkill /F /IM node.exe`, `python.exe`, `java.exe`, etc.).
- Always terminate processes precisely by PID and verified ownership (port, command line, or working directory).
- Before killing a process, first identify target PIDs and confirm they belong to this project task.
- Avoid commands that may impact unrelated apps or the coding agent itself; prefer the minimum-scope stop action.

## Project Docs Index
- Windows contributor prerequisites, local Tauri startup, and GitHub Windows release: [windows-contributor-guide.md](/e:/code/yzpd/workclaw/docs/development/windows-contributor-guide.md)

## Real Agent Eval Harness
- Real agent evals are local-only, manually triggered runtime regressions for validating real model + real skill execution without checking secrets into git.
- Keep scenario definitions in `agent-evals/scenarios/*.yaml` with anonymous `capability_id` values only. Do not store real skill paths, real API keys, or sensitive prompt internals in those tracked files.
- Keep real model/provider settings, real skill mappings, and external-system credentials only in `agent-evals/config/config.local.yaml`, which stays local and untracked.
- Use environment variable names in `config.local.yaml` such as `MINIMAX_API_KEY`; never paste raw API keys into the YAML file.
- Reports, traces, journals, and stdout/stderr artifacts are written to `temp/agent-evals/...` and stay local-only.
- First validated golden case:
  - Scenario: `pm_weekly_summary_xietao_2026_03_30_2026_04_04`
  - Prompt: `获取谢涛2026年3月30日到4月4日的工作日报并汇总成简报`
  - Expected route: `feishu-pm` family through the skill session runner, not `OpenTaskRunner`
  - Current verified baseline:
    - `status=pass`
    - `selected_skill=local-feishu-pm-hub`
    - `selected_runner=SkillSessionRunner`
    - `turn_count=1`
    - `tool_count=2`
    - `total_duration_ms≈39596`
    - `leaf_exec_duration_ms≈13258`

## Local Tauri Quick Start (Windows)
- Goal: launch the desktop window reliably for local testing.
- Run from repo root: `e:\code\yzpd\workclaw`.

### Start
```bash
pnpm install
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F
pnpm app
```

- `pnpm app` is the canonical cross-platform desktop dev entrypoint.
- The launcher now prefers the current shell environment. If `cargo` is not already on `PATH`, it will try `CARGO_HOME` / `RUSTUP_HOME` first and then fall back to `rustup which cargo`.
- On Windows, if Rust lives outside the default profile directory, set `CARGO_HOME` and `RUSTUP_HOME` before launching. The same `pnpm app` command should still be used afterward.
- If `pnpm install` fails with a pnpm store corruption error, recover with `pnpm install --force --store-dir .pnpm-store-local` and then rerun `pnpm app`.

### Verify
```bash
curl -I http://localhost:5174
tasklist | findstr /I runtime.exe
```

### Stop
```bash
netstat -ano | findstr LISTENING | findstr :5174
taskkill /PID <PID> /F
tasklist | findstr /I runtime.exe
taskkill /PID <RUNTIME_PID> /F
```

- If startup fails repeatedly, resolve port/process state first, then start once. Do not launch multiple `pnpm app` sessions in parallel.
