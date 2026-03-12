# AGENTS.md instructions for E:/code/yzpd/workclaw

## Skills
A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.

### Available skills
- skill-creator: Guide for creating effective skills. This skill should be used when users want to create a new skill (or update an existing skill) that extends Codex's capabilities with specialized knowledge, workflows, or tool integrations. (file: C:/Users/36443/.codex/skills/.system/skill-creator/SKILL.md)
- skill-installer: Install Codex skills into $CODEX_HOME/skills from a curated list or a GitHub repo path. Use when a user asks to list installable skills, install a curated skill, or install a skill from another repo (including private repos). (file: C:/Users/36443/.codex/skills/.system/skill-installer/SKILL.md)
- brainstorming: You MUST use this before any creative work - creating features, building components, adding functionality, or modifying behavior. Explores user intent, requirements and design before implementation. (file: D:/worksoftdata/.codex/superpowers/skills/brainstorming/SKILL.md)
- dispatching-parallel-agents: Use when facing 2+ independent tasks that can be worked on without shared state or sequential dependencies (file: D:/worksoftdata/.codex/superpowers/skills/dispatching-parallel-agents/SKILL.md)
- executing-plans: Use when you have a written implementation plan to execute in a separate session with review checkpoints (file: D:/worksoftdata/.codex/superpowers/skills/executing-plans/SKILL.md)
- finishing-a-development-branch: Use when implementation is complete, all tests pass, and you need to decide how to integrate the work - guides completion of development work by presenting structured options for merge, PR, or cleanup (file: D:/worksoftdata/.codex/superpowers/skills/finishing-a-development-branch/SKILL.md)
- receiving-code-review: Use when receiving code review feedback, before implementing suggestions, especially if feedback seems unclear or technically questionable - requires technical rigor and verification, not performative agreement or blind implementation (file: D:/worksoftdata/.codex/superpowers/skills/receiving-code-review/SKILL.md)
- requesting-code-review: Use when completing tasks, implementing major features, or before merging to verify work meets requirements (file: D:/worksoftdata/.codex/superpowers/skills/requesting-code-review/SKILL.md)
- subagent-driven-development: Use when executing implementation plans with independent tasks in the current session (file: D:/worksoftdata/.codex/superpowers/skills/subagent-driven-development/SKILL.md)
- systematic-debugging: Use when encountering any bug, test failure, or unexpected behavior, before proposing fixes (file: D:/worksoftdata/.codex/superpowers/skills/systematic-debugging/SKILL.md)
- test-driven-development: Use when implementing any feature or bugfix, before writing implementation code (file: D:/worksoftdata/.codex/superpowers/skills/test-driven-development/SKILL.md)
- using-git-worktrees: Use when starting feature work that needs isolation from current workspace or before executing implementation plans - creates isolated git worktrees with smart directory selection and safety verification (file: D:/worksoftdata/.codex/superpowers/skills/using-git-worktrees/SKILL.md)
- using-superpowers: Use when starting any conversation - establishes how to find and use skills, requiring Skill tool invocation before ANY response including clarifying questions (file: D:/worksoftdata/.codex/superpowers/skills/using-superpowers/SKILL.md)
- verification-before-completion: Use when about to claim work is complete, fixed, or passing, before committing or creating PRs - requires running verification commands and confirming output before making any success claims; evidence before assertions always (file: D:/worksoftdata/.codex/superpowers/skills/verification-before-completion/SKILL.md)
- writing-plans: Use when you have a spec or requirements for a multi-step task, before touching code (file: D:/worksoftdata/.codex/superpowers/skills/writing-plans/SKILL.md)
- writing-skills: Use when creating new skills, editing existing skills, or verifying skills work before deployment (file: D:/worksoftdata/.codex/superpowers/skills/writing-skills/SKILL.md)

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
