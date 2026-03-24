# Runtime Regression Repair Design

## Scope

Repair the frontend/runtime regressions uncovered by the March 24, 2026 verification sweep. The work covers three layers:

- TypeScript compile failures blocking `pnpm build:runtime`
- Frontend approval/risk-flow regressions blocking Vitest
- Feishu IM bridge behavior regressions blocking Vitest

## Goals

- Restore a clean `apps/runtime` TypeScript build.
- Preserve the intended full-access permission confirmation flow in Settings.
- Restore pending-approval rendering and cleanup behavior in ChatView.
- Restore Feishu IM bridge session refresh, delayed reply, token filtering, and retry-limit behavior.

## Non-Goals

- No release/versioning changes.
- No broad UI redesign.
- No sidecar or Rust protocol changes unless a frontend regression proves a contract mismatch.

## Recommended Approach

Use the smallest safe repair path:

1. Fix compile-time type drift first so the runtime can build again.
2. Repair risk-flow regressions next because they affect both UI behavior and tests.
3. Repair Feishu IM bridge regressions last, one failing behavior at a time, using the existing tests as the red/green harness.

This ordering reduces noise: the build must be green before we can trust runtime-level changes, and the risk-flow fixes are more localized than the Feishu bridge state machine.

## Affected Modules

- `apps/runtime/src/components/ChatView.tsx`
- `apps/runtime/src/components/SettingsView.tsx`
- `apps/runtime/src/components/settings/desktop/DesktopSettingsSection.tsx`
- `apps/runtime/src/scenes/chat/useChatStreamController.ts`
- `apps/runtime/src/__tests__/App.im-feishu-bridge.test.tsx`
- `apps/runtime/src/components/__tests__/ChatView.risk-flow.test.tsx`
- `apps/runtime/src/components/__tests__/SettingsView.risk-flow.test.tsx`
- `apps/runtime/src/components/__tests__/SettingsView.translation-preferences.test.tsx`

## Risks

- Some failing tests may reflect intentional copy changes rather than broken behavior.
- Feishu IM bridge failures may come from state sequencing, not just assertions.
- Changing shared settings/loading behavior can affect unrelated settings tests.

## Verification

- `pnpm --dir apps/runtime test`
- `pnpm build:runtime`
- Re-run narrower test files during red/green cycles before the full suite.
