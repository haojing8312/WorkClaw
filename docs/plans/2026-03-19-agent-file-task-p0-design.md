# Agent File Task P0 Design

**Context**

This design targets the WorkClaw runtime Tauri agent tool layer only. It addresses the concrete failure mode seen in the exported session where a document-reading task entered uncontrolled retries across `read`, Python, PowerShell, VBScript, and Node, then finally failed on a workspace-boundary violation.

**Scope**

- Tauri agent tools and tool execution context
- File-task preflight for local file operations
- Skill executability signaling
- Task-scoped temp working directory
- Safer command execution guidance at the tool layer

**Out of Scope**

- Adding native `.docx` parsing
- Sidecar changes
- Frontend UX changes
- New file-format-specific tools

---

## Problem Statement

The current runtime exposes file tools and shell execution tools, but it does not give the agent enough structured context to choose safe and reliable execution paths.

Observed gaps:

1. `read_file` blindly attempts UTF-8 reads, so binary/office files fail as generic decoding errors.
2. `skill_invoke` can load a skill that reads like it is actionable even when the skill has no executable tools in the current session.
3. `bash` accepts free-form shell strings, which makes Windows quoting, multiple interpreter environments, and path edge cases much more fragile.
4. The agent has no explicit task-scoped temp directory, so it may improvise paths outside the allowed workspace and get blocked.

These gaps cause wasted retries, misleading tool affordances, and poor failure localization.

---

## Goals

1. Stop binary/office file tasks from failing as raw UTF-8 read errors.
2. Make skill executability explicit before the agent treats a skill as an actionable path.
3. Give the agent a safe temp workspace for intermediate artifacts.
4. Add enough execution-capability metadata to guide shell/tool choices without a large refactor.

---

## Recommended Approach

Use a lightweight strategy layer injected into `ToolContext`, then teach a small number of tools to consume it.

### New Concepts

Add the following to the Tauri agent runtime:

- `TaskTempDir`: a per-run or per-task safe temporary directory created under a controlled root
- `ExecutionCaps`: lightweight environment facts such as preferred shell and detected interpreters
- `FileTaskCaps`: preflight classification for a requested file path
- `SkillExecutability`: explicit status for a resolved skill target

These are not standalone tools. They are execution metadata computed once and consumed by tools.

---

## Data Model Changes

Extend `ToolContext` with:

- `session_id: Option<String>`
- `task_temp_dir: Option<PathBuf>`
- `execution_caps: Option<ExecutionCaps>`
- `file_task_caps: Option<FileTaskCaps>`

Suggested supporting structs:

- `ExecutionCaps`
  - `platform`
  - `preferred_shell`
  - `python_candidates`
  - `node_candidates`
  - `notes`
- `FileTaskCaps`
  - `requested_path`
  - `resolved_path`
  - `exists`
  - `extension`
  - `read_mode`
  - `reason`
- `SkillExecutability`
  - `skill_name`
  - `declared_tools`
  - `narrowed_tools`
  - `status`
  - `reason`

`read_mode` should be a small enum-like string set:

- `text_direct`
- `binary_or_office`
- `missing`
- `unknown`

`status` should be:

- `executable`
- `instruction_only`
- `blocked`

---

## Behavior Changes

### 1. File-task preflight

Add a small helper that classifies a path before `read_file` attempts text decoding.

Rules:

- If file extension is clearly text-like, allow normal text read.
- If file extension is office/binary-like such as `.docx`, `.pdf`, `.xlsx`, `.pptx`, `.zip`, classify as `binary_or_office`.
- If file does not exist, classify as `missing`.

`read_file` behavior after this change:

- `text_direct`: continue as today
- `binary_or_office`: return a structured failure explaining that raw text read is not the correct operation
- `missing`: return existing missing-file style failure

This avoids misleading UTF-8 decode errors for office files.

### 2. Skill executability signaling

Keep the existing `skill_invoke` loading behavior, but add explicit executability metadata in the returned content payload shape or text summary.

Rules:

- No declared tools: mark `instruction_only`
- Declared tools present and narrowed tools available: mark `executable`
- Declared tools present but none remain after narrowing: mark `blocked`

This does not prevent loading the skill text, but it makes the runtime state explicit and machine-readable enough for the agent to behave better.

### 3. Task-scoped temp directory

Create a temp directory for the run when building `ToolContext`.

Requirements:

- Directory name should include a WorkClaw prefix plus session or run identity
- It must live under a controlled base such as `std::env::temp_dir()`
- It should be exposed in `ToolContext`

Initial P0 usage:

- `write_file` may optionally redirect specially-marked intermediate paths later, but for now the main requirement is to expose a safe place to use
- `bash` and future orchestration logic can reference it in error messages and structured details

### 4. Safer execution guidance

Do not replace `bash` with a whole new process layer in this P0.

Instead:

- Add execution-capability info to `ToolContext`
- Return it from helper logic for diagnostics
- Tighten `bash` messaging and metadata so the agent gets a clearer signal about shell/platform context

This keeps the code change small while preparing for a later structured exec migration.

---

## Files Likely To Change

- `apps/runtime/src-tauri/src/agent/types.rs`
- `apps/runtime/src-tauri/src/agent/executor.rs`
- `apps/runtime/src-tauri/src/agent/tools/read_file.rs`
- `apps/runtime/src-tauri/src/agent/tools/write_file.rs`
- `apps/runtime/src-tauri/src/agent/tools/skill_invoke.rs`
- `apps/runtime/src-tauri/src/agent/tools/bash.rs`
- `apps/runtime/src-tauri/src/agent/tools/mod.rs`

Suggested new helper files:

- `apps/runtime/src-tauri/src/agent/file_task_preflight.rs`
- `apps/runtime/src-tauri/src/agent/execution_caps.rs`

---

## Risks

1. `ToolContext` changes may ripple across tests and tool construction.
2. `read_file` behavior changes could affect workflows that currently rely on raw failure text.
3. `skill_invoke` output changes must preserve current human-readable behavior while adding more explicit status.
4. Temp-dir creation should not leak on failed runs; cleanup can remain best-effort for P0.

---

## Smallest Safe Path

1. Extend `ToolContext`
2. Add helper modules for file preflight and execution caps
3. Inject the new fields in `executor`
4. Update `read_file`
5. Update `skill_invoke`
6. Add temp-dir details to `write_file`/`bash` result metadata where helpful
7. Add focused tests for the new behavior

---

## Verification

Minimum verification for this design when implemented:

- Rust unit tests for file preflight helper
- Rust unit tests for skill executability classification
- Rust unit tests for `ToolContext` temp-dir injection and path handling
- Targeted tests for `read_file` on text vs `.docx`-like inputs

