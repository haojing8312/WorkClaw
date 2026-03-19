# Session Runtime State Persistence Design

**Date:** 2026-03-19

## Summary

Fix the bug where switching sidebar sessions or task tabs drops in-progress chat output from the UI even though the backend run continues.

## Problem

`ChatView` keeps in-progress execution state in component-local React state. When the user switches away and comes back, the component resets that local state and only reloads persisted messages plus limited `session_runs.buffered_text` recovery. This drops streamed text, tool call progress, reasoning state, and other runtime-only UI state.

## Scope

- Preserve visible in-progress runtime state per `sessionId` across:
  - sidebar session switches
  - task tab switches
  - temporary navigation away from `start-task`
- Keep current backend storage contract unchanged
- Reuse existing `session_runs` / `session_run_events` recovery as a fallback, not the primary source for active UI restoration

## Chosen Approach

Store a lightweight per-session runtime UI snapshot in `App` and pass it into `ChatView` when that session becomes active again.

`ChatView` will:
- emit runtime-state updates upward whenever visible execution state changes
- hydrate local execution state from the saved snapshot on mount/session switch
- continue to clear the snapshot naturally when the run finishes and local runtime state becomes empty

## Runtime State To Persist

- `streaming`
- `streamItems`
- `streamReasoning`
- `agentState`
- `subAgentBuffer`
- `subAgentRoleName`
- `mainRoleName`
- `mainSummaryDelivered`
- `delegationCards`

## Risks

- Late async loads could overwrite restored UI state after a switch
- Persisting too much state could preserve stale visuals after completion

## Mitigations

- Keep persisted runtime state scoped to in-progress UI only
- Let normal completion paths publish an empty runtime snapshot
- Add regression tests for both `ChatView` hydration and `App`-level switching flows
