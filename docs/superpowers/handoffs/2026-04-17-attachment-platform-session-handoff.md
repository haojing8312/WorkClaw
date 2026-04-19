# WorkClaw Attachment Platform Session Handoff

Date: 2026-04-17

## Update 2026-04-19 (Final Session Close-Out)

This handoff is now historical context rather than an active execution baseline.

The attachment-platform line and its follow-on IM host alignment work have been implemented, split, verified, and committed in the main repo history.

As of the latest 2026-04-19 closeout docs, the recommended final Phase 3 status is:

- `Phase 3 complete with known Windows runtime_lib libtest caveat`

That caveat is environmental rather than architectural. On the current main Windows development path, execution-level verification is complete and repeatable.

### Landed commit sequence

- `e73e311` `feat(attachments): build unified attachment platform`
- `bb60f0c` `feat(im-host): extract shared IM host runtime platform`
- `72ffee1` `feat(settings): align IM channels with host registry`
- `13c9ae8` `feat(employees): align session launch with IM registry`
- `f27e798` `docs(im-host): add phase 3 acceptance summary`
- `760d90e` `chore(im-host): add phase 3 verification runner`
- `749277d` `test(im-host): cover unified WeCom reply dispatch`
- `d02012e` `test(im-host): add windows-safe phase 3 regressions`
- `b9c19a0` `docs(handoff): close out attachment platform session`
- `3d60de3` `docs(im-host): tighten phase 3 closeout evidence`
- `5a805b8` `docs(im-host): add phase 3 final status draft`
- `cb46bc2` `docs(im-host): prefill phase 3 verification result`

### What is now true

- the unified frontend attachment policy and draft layer are present
- the backend attachment policy / validation / resolution path is present
- audio transcription and video fallback handling have landed
- team-entry / employee-entry session launch now preserves initial attachments
- settings and runtime status now align to the shared IM host registry
- WorkClaw now has a Windows-safe Phase 3 IM host regression target:
  - `pnpm test:im-host-windows-regression`
- full repo-level Phase 3 verification now runs on the current Windows machine:
  - `pnpm verify:openclaw-im-host:phase3`
- the final closeout docs now explicitly say the main delivery path is done, with only a known Windows `runtime_lib` libtest caveat left for supplementary proof

### Most relevant verification that actually passed

- `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx`
- `cargo check -p runtime`
- `pnpm test:rust-fast`
- `pnpm test:im-host-windows-regression`
- `pnpm verify:openclaw-im-host:phase3`
- `pnpm --dir apps/runtime exec vitest run ./plugin-host/src/runtime.test.ts`

### Remaining truth

- the old `runtime_lib` libtest binary path can still hit Windows `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` in this environment
- that environment issue is no longer the blocker for Phase 3 verification, because the repo now contains and uses a Windows-safe regression path
- authoritative current status and verification guidance now live in:
  - `docs/architecture/openclaw-im-host/06-phase-3-acceptance-summary.md`
  - `docs/architecture/openclaw-im-host/07-phase-3-external-verification-runbook.md`
  - `docs/architecture/openclaw-im-host/08-phase-3-external-verification-result-template.md`
  - `docs/architecture/openclaw-im-host/09-phase-3-closeout-checklist.md`
  - `docs/architecture/openclaw-im-host/10-phase-3-final-status-draft.md`

### Recommended handoff wording from this point on

Use this summary when referring to the status of this work:

- the attachment-platform objective is complete in repo history
- the IM host Phase 3 objective is complete in the main Windows delivery path
- the current best final label is `Phase 3 complete with known Windows runtime_lib libtest caveat`
- the only meaningful follow-up is supplementary proof on a machine where the legacy `cargo test --lib ...` route is stable

## Update 2026-04-19

Correction to the optimistic 2026-04-18 narrative above:

- the current repo still does **not** contain the planned attachment-platform foundation files such as `attachmentPolicy.ts`, `attachmentDrafts.ts`, Rust attachment policy/validation/resolution modules, or `test_chat_attachment_platform.rs`
- the current workspace still contains the legacy narrow attachment flow in:
  - `apps/runtime/src/lib/chatAttachments.ts`
  - `apps/runtime/src/scenes/chat/useChatDraftState.ts`
  - `apps/runtime/src/components/NewSessionLanding.tsx`
- the old P0/P1 plan file reflects the intended target architecture, not the actual landed state

Authoritative remaining-work plan:

- `docs/superpowers/plans/2026-04-19-workclaw-attachment-platform-remaining-implementation.md`

Treat that new plan as the real execution baseline from this point onward.

## Update 2026-04-18

This handoff is no longer at the "install blocked, implementation not started" stage.

Attachment platform `P0 + P1` implementation work has already landed in the active worktree, including:

- unified frontend attachment model and send contract
- Tauri `AttachmentInput` plus `SendMessagePart::Attachment`
- backend attachment policy, validation, and resolution path
- transcript preservation for attachment parts
- explicit OpenAI adapter attachment handling
- frontend and Rust attachment-platform regression coverage

Additional follow-up completed on 2026-04-18:

- added/kept frontend regression coverage for new-session attachment bootstrap
- fixed Rust lib-test compile drift in:
  - `apps/runtime/src-tauri/src/agent/runtime/child_session_runtime.rs`
  - `apps/runtime/src-tauri/src/agent/runtime/skill_routing/index.rs`
  - `apps/runtime/src-tauri/src/agent/runtime/skill_routing/observability.rs`
  - `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
  - `apps/runtime/src-tauri/src/commands/employee_agents/group_run_execution_service.rs`
  - `apps/runtime/src-tauri/src/commands/employee_agents/memory_commands.rs`
- fixed integration-test drift in:
  - `apps/runtime/src-tauri/tests/test_chat_repo.rs`
  - `apps/runtime/src-tauri/tests/test_skill_commands.rs`
  - `apps/runtime/src-tauri/tests/test_approval_bus.rs`
  - `apps/runtime/src-tauri/src/commands/skills/local_skill_service.rs`
  - `apps/runtime/src-tauri/src/commands/skills.rs`
  - `apps/runtime/src-tauri/src/agent/runtime/mod.rs`
- `cargo test --lib --no-run` now finishes successfully for the runtime lib test target
- `pnpm test:rust-fast` passes
- targeted integration tests now pass:
  - `cargo test --test test_chat_repo`
  - `cargo test --test test_skill_commands`
  - `cargo test --test test_approval_bus`

## Latest Verification Snapshot

Most recent attachment-platform-focused verification completed in this branch:

- `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx`
  - result: pass
  - coverage signal: attachment intake UI, side-panel attachment flow, new-session first-send bootstrap
- `pnpm test:rust-fast`
  - result: pass
  - coverage signal: repo Rust fast-path baseline still healthy after attachment-related Rust changes
- `cargo test --lib --no-run`
  - result: pass
  - coverage signal: runtime lib test target compiles after test drift fixes
- `node scripts/run-cargo-isolated.mjs attachment-transcript -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
  - result: pass
  - coverage signal: isolated Windows compile gate for transcript/OpenAI attachment work without shared target-dir contamination
- `cargo test --test test_chat_repo`
  - result: pass
- `cargo test --test test_skill_commands`
  - result: pass
- `cargo test --test test_approval_bus`
  - result: pass

Important note:

- `cargo test --test test_chat_attachment_platform` is not runnable in the current repo state because there is no test target with that name. Treat this as "not applicable in current tree", not as a failing attachment regression.

Current remaining blockers are no longer attachment-platform compile errors. The main unresolved items are:

- repo-wide Rust warnings and unrelated integration-test drift in other surfaces
- targeted `cargo test --lib <filter>` execution aborting in this environment with Windows `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` even after the lib test target compiles successfully
- isolated `--no-run` compilation remains workable on this machine even when direct libtest execution does not; prefer `node scripts/run-cargo-isolated.mjs <label> -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run` for narrow Rust compile verification
- this `0xc0000139` issue is older than the current attachment work: the older `runtime_lib-21590eb8de04bfe6.exe` unit-test harness reproduces the same startup failure, while integration-test executables and `runtime.exe` start normally
- the older handoff sections below are now historical context, not the current branch state

## Task Summary

User wants WorkClaw attachment upload experience upgraded to align with OpenClaw, not just patched superficially.

The agreed direction is:

- Benchmark and align against OpenClaw
- Build a unified runtime-level attachment platform
- First implementation scope should target `P0 + P1`
- Use subagent-driven execution for implementation

## Research Conclusions Already Reached

### Current WorkClaw problems

- Current attachment capability is effectively limited to `image`, `text-file`, and `pdf-file`
- Limits are mostly hardcoded in frontend code and diverge across entry points
- Only images remain native multimodal inputs by runtime/adapters
- Text and PDF attachments are flattened into prompt text too early
- Team-entry session launch currently drops attachments
- File input UX is weak, including missing `accept` and `alert`-driven rejection behavior

Key files identified during analysis:

- `apps/runtime/src/lib/chatAttachments.ts`
- `apps/runtime/src/scenes/chat/useChatDraftState.ts`
- `apps/runtime/src/components/NewSessionLanding.tsx`
- `apps/runtime/src/scenes/chat/useChatSendController.ts`
- `apps/runtime/src-tauri/src/commands/chat.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- `apps/runtime/src-tauri/src/adapters/openai.rs`

### OpenClaw benchmark conclusions

- OpenClaw has a much richer attachment/media direction
- It supports broader capability categories such as `image`, `audio`, `video`
- It has a more structured policy layer including ideas like `maxBytes`, `maxChars`, `maxAttachments`, `mode`, `prefer`
- It preserves attachment/media semantics deeper into runtime instead of flattening everything early
- It has richer support for multiple source types such as local path, URL, data URL, base64, etc.

Important reference files:

- `references/openclaw/src/gateway/chat-attachments.ts`
- `references/openclaw/src/gateway/server-methods/attachment-normalize.ts`
- `references/openclaw/docs/gateway/openresponses-http-api.md`
- `references/openclaw/docs/channels/msteams.md`
- `apps/runtime/sidecar/vendor/openclaw-core/src/config/types.tools.ts`

## Design Decisions Confirmed With User

These were explicitly agreed in the session:

1. Do not do a shallow UI-only fix
2. Align to OpenClaw direction
3. Include `audio` and `video` in the platform design, not only stronger document support
4. Use a runtime-first attachment platform approach
5. Split delivery into `P0`, `P1`, `P2`
6. First implementation should target `P0 + P1`
7. Implementation execution style should be `subagent-driven-development`

## Spec Created

Spec file:

- `docs/superpowers/specs/2026-04-16-workclaw-attachment-platform-design.md`

Git commit for spec:

- `6c85d3b` with message: `docs: add attachment platform design spec`

Spec contents already include:

- problem statement
- goals/non-goals
- capability model
- layered attachment data model
- policy layer
- runtime pipeline
- provider semantics
- phased delivery (`P0`, `P1`, `P2`)
- testing strategy

## Implementation Plan Created

Plan file:

- `docs/superpowers/plans/2026-04-16-workclaw-attachment-platform-p0-p1.md`

This plan was written but not committed during the session.

The plan currently includes:

- file map
- Task 1: frontend attachment policy
- Task 2: frontend attachment draft normalization
- Task 3: unify chat + landing attachment intake
- Task 4: expand session launch + send request contract
- Task 5: authoritative backend policy + validation
- Task 6: runtime resolution layer
- Task 7: preserve attachment semantics through transcript
- Task 8: adapter support matrix + explicit OpenAI handling
- Task 9: regression + WorkClaw verification

## Worktree / Branch State

An isolated worktree was created successfully.

Worktree path:

- `D:/code/WorkClaw/.worktrees/attachment-platform-p0-p1`

Branch created for implementation:

- `feat/attachment-platform-p0-p1`

The `.worktrees/` directory is already ignored by git in the repo root `.gitignore`, so no ignore fix was needed.

## Current Blocker

The session was in the middle of preparing the worktree baseline when dependency installation hit a network failure.

Attempted command:

```bash
pnpm install
```

Run location:

- `D:/code/WorkClaw/.worktrees/attachment-platform-p0-p1`

Observed outcome:

- install progressed substantially
- then failed with npm registry fetch `ECONNRESET`
- specific package mentioned in failure output: `lucide-react-0.469.0.tgz`

This means the worktree setup is incomplete. No implementation task has started yet.

## Recommended Resume Point For Next Session

Start in the worktree:

```bash
cd D:/code/WorkClaw/.worktrees/attachment-platform-p0-p1
```

Then continue from here:

1. Retry dependency install:

```bash
pnpm install
```

If npm registry remains flaky, retry once or use the repo’s normal recovery approach if needed.

2. After install succeeds, run a minimal baseline verification before implementation:

```bash
git status --short
pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx
```

Optional Rust baseline if needed:

```bash
cargo test --test test_session_run_commands
```

3. Then begin subagent-driven execution of the implementation plan from Task 1.

## Immediate Next Task

Per the agreed workflow, the next implementation task should be:

### Task 1: Introduce Frontend Attachment Policy

Files:

- Create: `apps/runtime/src/lib/attachmentPolicy.ts`
- Create: `apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts`
- Modify: `apps/runtime/src/lib/chatAttachments.ts`

Goal of Task 1:

- replace frontend magic numbers with shared OpenClaw-aligned policy defaults
- add `buildFileInputAccept()`
- add capability-based defaults for `image`, `audio`, `video`, `document`

## Important Constraints To Preserve

- Do not skip the runtime-first architecture direction
- Do not collapse the work into a UI-only file type expansion
- Keep compatibility with legacy stored message/session content
- Backend validation must become authoritative
- Provider handling must become explicit; no silent attachment loss
- Preserve the phased scope: implement `P0 + P1`, keep `P2` as target architecture, not first delivery

## Suggested Prompt For The Next Session

Use something like:

> Continue the WorkClaw attachment platform task from `docs/superpowers/handoffs/2026-04-17-workclaw-attachment-platform-session-handoff.md`. Use the existing spec at `docs/superpowers/specs/2026-04-16-workclaw-attachment-platform-design.md` and plan at `docs/superpowers/plans/2026-04-16-workclaw-attachment-platform-p0-p1.md`. Resume in worktree `D:/code/WorkClaw/.worktrees/attachment-platform-p0-p1` on branch `feat/attachment-platform-p0-p1`, finish baseline setup, then execute Task 1 using subagent-driven-development.
