# Skill Creator Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh WorkClaw's built-in skill creation guidance so local skill generation emphasizes trigger quality and lightweight evaluation, not just valid structure.

**Architecture:** Keep the runtime unchanged and update the built-in markdown/template assets that it already embeds. Protect the new requirements with string-level Rust tests in the builtin skill registry test module.

**Tech Stack:** Rust, markdown assets, Tauri runtime tests

---

### Task 1: Add failing tests for the refreshed builtin guidance

**Files:**
- Modify: `apps/runtime/src-tauri/src/builtin_skills.rs`
- Test: `apps/runtime/src-tauri/src/builtin_skills.rs`

**Step 1: Write the failing test**

Add assertions that the built-in skill creator markdown includes:
- reuse / existing skill check
- trigger examples
- anti-examples or non-trigger guidance
- lightweight evaluation prompts

Add assertions that the guide includes:
- optional advanced frontmatter guidance
- trigger-quality iteration

Add assertions that the local template includes:
- `When Not to Use`
- `Prompt Examples`

**Step 2: Run test to verify it fails**

Run: `cargo test builtin_skill --manifest-path apps/runtime/src-tauri/Cargo.toml`

Expected: FAIL because current markdown/template does not contain the new guidance.

**Step 3: Write minimal implementation**

Update the markdown/template files referenced by `builtin_skills.rs` until the assertions pass.

**Step 4: Run test to verify it passes**

Run: `cargo test builtin_skill --manifest-path apps/runtime/src-tauri/Cargo.toml`

Expected: PASS

### Task 2: Refresh built-in skill creator content

**Files:**
- Modify: `apps/runtime/src-tauri/builtin-skills/skill-creator/SKILL.md`
- Modify: `apps/runtime/src-tauri/builtin-skills/skill-creator-guide/SKILL.md`
- Modify: `apps/runtime/src-tauri/builtin-skills/skill-creator-guide/templates/LOCAL_SKILL_TEMPLATE.md`

**Step 1: Update the authoring skill**

Keep it concise, but add:
- create vs reuse decision
- description trigger optimization
- positive / negative prompt examples
- lightweight evaluation loop

**Step 2: Update the internal guide**

Align local scaffold guidance with runtime capability and refreshed best practices.

**Step 3: Update the local template**

Add sections that steer users toward better trigger coverage and evaluation.

**Step 4: Run tests**

Run: `cargo test builtin_skill --manifest-path apps/runtime/src-tauri/Cargo.toml`

Expected: PASS

### Task 3: Final verification

**Files:**
- Modify: none

**Step 1: Run targeted verification**

Run: `cargo test builtin_skill --manifest-path apps/runtime/src-tauri/Cargo.toml`

Expected: PASS with no assertion failures.

**Step 2: Inspect git diff**

Run: `git diff -- apps/runtime/src-tauri/src/builtin_skills.rs apps/runtime/src-tauri/builtin-skills/skill-creator/SKILL.md apps/runtime/src-tauri/builtin-skills/skill-creator-guide/SKILL.md apps/runtime/src-tauri/builtin-skills/skill-creator-guide/templates/LOCAL_SKILL_TEMPLATE.md docs/plans/2026-03-11-skill-creator-refresh-design.md docs/plans/2026-03-11-skill-creator-refresh-plan.md`

Expected: Only the planned files changed.
