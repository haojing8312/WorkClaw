# Session Draft Isolation Design

**Date:** 2026-03-14

**Goal:** Make unsent composer text behave like browser tabs: each chat session keeps its own local draft when the user switches away and comes back.

## Scope

- Persist unsent text drafts by `session_id`
- Restore the matching draft when `ChatView` switches sessions
- Clear the draft after a successful send or when a session is auto-started with an initial message
- Keep the feature frontend-only and local-only

## Non-Goals

- No backend storage
- No draft sync across devices
- No attachment draft persistence
- No visible draft management UI

## Approach

Store the composer text in `localStorage` under a session-scoped key. `ChatView` stays the single owner of the composer input, so it can save drafts on text changes and restore them on `sessionId` changes without widening the data flow through `App`.

This design keeps the product mental model simple:

- switching sessions preserves unfinished work
- sending a message still clears the composer
- new sessions with an initial auto-sent prompt do not resurrect an old stale draft

## Edge Cases

- Switching sessions must not write session A's draft into session B's key during the same render cycle
- Empty drafts remove their storage entry instead of storing blank values
- `localStorage` failures should be ignored silently so chat still works

## Validation

- Component test proving session A and session B restore different drafts
- Existing resilience tests for initial auto-send still pass
- TypeScript still passes
