# Rust AI Guardrails Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce concise root guidance and Rust-specific AI development guardrails for `apps/runtime/src-tauri/` without changing runtime behavior.

**Architecture:** Keep the root `AGENTS.md` short and cross-repo, then add a local `apps/runtime/src-tauri/AGENTS.md` that owns Rust runtime layering and file-budget rules. Reference the new guidance from the root file so future agent sessions load the right constraints close to the code being edited.

**Tech Stack:** Markdown docs, repo-local `AGENTS.md`, existing WorkClaw workflow skills.

---

### Task 1: Add the validated design record

**Files:**
- Create: `docs/plans/2026-03-23-rust-ai-guardrails-design.md`

**Step 1: Write the design doc**

Write the agreed Rust AI guardrail design with:
- threshold rationale for `500 / 800`
- command/service/repo/gateway landing-zone guidance
- anti-micro-file guidance
- root-vs-local guidance file split

**Step 2: Review the document for scope discipline**

Confirm it does not silently expand into code refactor work or CI automation.

**Step 3: Commit**

```bash
git add docs/plans/2026-03-23-rust-ai-guardrails-design.md
git commit -m "docs: add rust ai guardrails design"
```

### Task 2: Add a Rust runtime local guidance file

**Files:**
- Create: `apps/runtime/src-tauri/AGENTS.md`

**Step 1: Draft local guidance**

Include:
- scope of the Rust runtime area
- module landing zones
- file-budget thresholds
- when large files may still accept bug fixes
- SQLite compatibility reminder
- verification command reminders relevant to Rust work

**Step 2: Review for brevity**

Confirm the file is concise enough to serve as agent memory, not an architecture book.

**Step 3: Commit**

```bash
git add apps/runtime/src-tauri/AGENTS.md
git commit -m "docs: add rust runtime agent guardrails"
```

### Task 3: Link the new local guidance from the root file

**Files:**
- Modify: `AGENTS.md`

**Step 1: Add a short Rust guidance section**

Add only the minimal cross-repo text needed to:
- say Rust runtime work should follow the closer local guidance in `apps/runtime/src-tauri/AGENTS.md`
- mention the line-budget trigger model at a high level

**Step 2: Review for duplication**

Remove or avoid repeated Rust details that now belong in the local guidance file.

**Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: route rust work to local runtime guidance"
```

### Task 4: Verify the guidance set

**Files:**
- Verify: `AGENTS.md`
- Verify: `apps/runtime/src-tauri/AGENTS.md`
- Verify: `docs/plans/2026-03-23-rust-ai-guardrails-design.md`
- Verify: `docs/plans/2026-03-23-rust-ai-guardrails-plan.md`

**Step 1: Check file diffs**

Run:

```bash
git diff -- AGENTS.md apps/runtime/src-tauri/AGENTS.md docs/plans/2026-03-23-rust-ai-guardrails-design.md docs/plans/2026-03-23-rust-ai-guardrails-plan.md
```

Expected: only documentation and guidance changes

**Step 2: Sanity-check markdown**

Run:

```bash
Get-Content AGENTS.md
Get-Content apps/runtime/src-tauri/AGENTS.md
```

Expected: guidance is readable, short, and non-contradictory

**Step 3: Commit**

```bash
git add AGENTS.md apps/runtime/src-tauri/AGENTS.md docs/plans/2026-03-23-rust-ai-guardrails-design.md docs/plans/2026-03-23-rust-ai-guardrails-plan.md
git commit -m "docs: establish rust ai development guardrails"
```
