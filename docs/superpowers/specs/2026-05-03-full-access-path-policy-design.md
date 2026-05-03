# Full Access Path Policy Design

Date: 2026-05-03

## Strategy Summary

- Change surface: runtime tool path authorization for file tools, especially `read_file`, `write_file`, and any tool that calls `ToolContext::check_path`.
- Affected modules: `apps/runtime/src-tauri/src/agent/types.rs`, `apps/runtime/src-tauri/src/agent/context.rs`, runtime call sites that build `ToolContext`, targeted Rust tests, and the desktop runtime permission-mode copy in `apps/runtime/src/components/settings/desktop/`.
- Main risk: widening file-tool access can accidentally allow destructive writes to credentials, shell profiles, app config, or repository metadata.
- Recommended smallest safe path: keep `work_dir` as cwd/session context, add a separate path-access policy derived from `permission_mode`, and let `full_access` allow ordinary absolute paths outside the session directory while retaining sensitive-path guards.
- Required verification: targeted Rust tests for path policy and file tools, plus the WorkClaw Rust fast path if the touched surface compiles beyond the focused tests.
- Release impact: user-visible behavior change for `full_access`; no installer, schema, vendor lane, or packaging change expected.

## Background

WorkClaw currently treats the selected session directory as both the agent working directory and the hard file-tool boundary. Shell execution mostly uses it as `current_dir`, but file tools call `ToolContext::check_path`, which rejects paths outside `work_dir`. This creates a mismatch: `full_access` bypasses approval decisions through `PermissionMode::Unrestricted`, but it still cannot read or write ordinary files outside the session directory.

The reference systems we inspected separate these concepts:

- close code keeps a workspace concept but supports additional directories and a bypass mode for broader task execution.
- openclaw treats workspace as cwd/context while real isolation is handled by sandbox backends and explicit mounts.
- hermes uses cwd as execution context and only enables a write-safe root when explicitly configured.

For WorkClaw, the requested behavior is direct: when a session is in `full_access`, file tools should be allowed to access ordinary paths outside the selected session directory.

## Goals

- Preserve the selected session directory as the default cwd and default relative-path base.
- Make `full_access` mean file tools can read and write ordinary absolute paths outside `work_dir`.
- Keep `standard` and `accept_edits` behavior workspace-only for compatibility.
- Protect obviously sensitive paths even in `full_access`.
- Avoid database schema changes and avoid changing sidecar or shell execution semantics in this phase.

## Non-Goals

- Do not implement an additional-directory allowlist UI in this phase.
- Do not introduce OS-level sandboxing, Docker mounts, or a sidecar permission protocol.
- Do not rework all approval policy classification into path authorization in this phase.
- Do not make relative paths escape the session directory.

## Proposed Architecture

Split `ToolContext` responsibilities into two concepts:

1. `work_dir`: the session working directory and relative-path base.
2. `path_access`: the file-tool path authorization mode.

The Rust runtime can model this with an enum near `ToolContext`:

```rust
pub enum PathAccessPolicy {
    WorkspaceOnly,
    FullAccessWithSensitiveGuards,
}
```

`ToolContext::check_path` continues to normalize paths for file tools, but its authorization decision changes:

- Relative path: resolve under `work_dir`, then apply the selected policy.
- Absolute path inside `work_dir`: allow under both policies.
- Absolute path outside `work_dir`:
  - `WorkspaceOnly`: reject, matching today's behavior.
  - `FullAccessWithSensitiveGuards`: allow unless the path is classified as sensitive.

The policy is derived from the session permission mode:

- `standard`: `WorkspaceOnly`
- `accept_edits`: `WorkspaceOnly`
- `default`: normalize through the current existing permission-mode path, then use the resulting concrete mode.
- `full_access` / `unrestricted`: `FullAccessWithSensitiveGuards`

## Sensitive Path Guard

Phase 1 should use a simple hard-deny guard inside the path-access layer. This is intentionally conservative because `ToolContext::check_path` runs after the approval decision path. Moving these checks into interactive approval can be a later refinement.

The guard should block common credential and runtime-control surfaces, including:

- `.ssh`, `.aws`, `.azure`, `.kube`, `.gnupg`, and similar credential directories.
- `.env`, `.env.*`, `*.pem`, `*.key`, and common token or credential files.
- shell startup files such as `.bashrc`, `.zshrc`, PowerShell profile files, and Windows startup folders.
- repository metadata such as `.git` internals.
- WorkClaw runtime database/config locations if they are easy to identify from existing app paths.

This guard is not a complete malware sandbox. It is a pragmatic protection against the most likely accidental high-impact operations while still letting the model complete ordinary tasks such as installing Python packages, writing artifacts in temp directories, updating external project files, and reading tool configuration files that are not credential stores.

## Data Flow

1. Session creation and runtime preferences continue to store `permission_mode` as they do today.
2. Turn preparation parses `full_access` and `unrestricted` into `PermissionMode::Unrestricted`.
3. Runtime call sites that build `ToolContext` pass either `PermissionMode` or a derived `PathAccessPolicy`.
4. `build_tool_context` stores both `work_dir` and `path_access` on `ToolContext`.
5. File tools call `ctx.check_path`.
6. `check_path` resolves the path and applies `path_access`.
7. Shell tools keep using `work_dir` as `current_dir`; no new filesystem restriction is added to shell execution in this phase.

## Affected Code

- `apps/runtime/src-tauri/src/agent/types.rs`: add `PathAccessPolicy`, update `ToolContext`, and move path authorization out of a hard workspace-only assumption.
- `apps/runtime/src-tauri/src/agent/context.rs`: update `build_tool_context` to accept or derive a path policy.
- `apps/runtime/src-tauri/src/agent/runtime/kernel/lane_executor.rs`, `direct_dispatch.rs`, `session_runtime.rs`, and `turn_executor.rs`: pass permission context into tool-context construction where needed.
- `apps/runtime/src-tauri/src/agent/file_task_preflight.rs`, `tools/read_file.rs`, `tools/write_file.rs`, and `tools/edit.rs`: should keep using `check_path`; they should not each duplicate policy logic.
- `apps/runtime/src/components/settings/desktop/DesktopRuntimeSection.tsx`: update `full_access` copy so users understand file tools can access normal paths outside the session directory.

If the sensitive-path helper grows beyond a few straightforward checks, it should live in a small new module such as `apps/runtime/src-tauri/src/agent/path_access.rs` instead of bloating `types.rs`.

## Error Handling

Path rejection should explain the reason clearly:

- Workspace mode outside path: "路径不在当前会话目录内；切换到 full_access 后可访问普通外部路径。"
- Sensitive path in full access: "full_access 仍会保护敏感路径，拒绝访问该位置。"
- Path normalization or canonicalization failure: preserve the existing error style, but include the target path when safe.

The first implementation can hard-deny sensitive paths. A future approval-based version can classify these as critical-risk decisions before tool execution.

## Compatibility

Existing sessions keep their stored `permission_mode`. No migration is needed.

Existing standard-mode behavior remains unchanged. Existing full-access sessions become more permissive for file tools, which matches the explicit user intent and the label users already see.

Relative paths still resolve under `work_dir`, so model outputs like `write_file("report.md")` do not drift into the process cwd or arbitrary directories.

## Testing Plan

Add focused Rust coverage for the policy boundary:

- `WorkspaceOnly` rejects an absolute path outside `work_dir`.
- `FullAccessWithSensitiveGuards` allows an ordinary absolute path outside `work_dir`.
- `FullAccessWithSensitiveGuards` rejects a representative sensitive path such as `.ssh/config`, `.env`, or `.git/config`.
- Relative paths still resolve inside `work_dir`.
- `write_file` can write to an ordinary outside temp path when the context is full access.

Run targeted tests first, then the repo's Rust fast path if compile impact crosses shared runtime modules:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml test_write_file -- --nocapture
pnpm test:rust-fast
```

If the settings copy changes in a component with existing tests, run the relevant frontend test file or the smallest Vitest command that covers it.

## Rollout

This can ship as a single runtime change without a feature flag because it only changes `full_access`, a mode that already communicates higher trust. The release note should call out that `full_access` now permits file tools to read and write ordinary paths outside the session directory, while sensitive-path guards remain in place.

## Open Decisions Resolved

- Full access should directly allow file tools to access ordinary paths outside the session directory.
- Additional directory allowlists are not part of phase 1.
- Sensitive paths are hard-denied first; approval-routing for sensitive paths is deferred.
