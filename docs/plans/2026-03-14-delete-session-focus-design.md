# Delete Session Focus Design

**Date:** 2026-03-14

**Goal:** Make closing the current session feel like closing a browser tab: move focus to a nearby session instead of dropping the user back to the landing state whenever possible.

## Scope

- Only affects deleting the currently selected session
- Focus the next session in the sidebar order first
- If there is no next session, focus the previous one
- If no sessions remain, return to the landing state

## Non-Goals

- No change to the delete API
- No delete confirmation UX changes
- No session sorting changes
- No cross-skill grouping or filtering logic

## Approach

Use the current in-memory `sessions` order in `App.tsx` as the source of truth for adjacency. When deleting the selected session, compute the fallback session id before removing it, update local state immediately, and then refresh from `list_sessions` to stay aligned with persisted data.

This keeps the behavior simple and responsive:

- closing the current session keeps you inside chat if another session exists
- deleting a non-selected session does not disturb the current focus
- sidebar order remains the same mental model users already see

## Edge Cases

- Deleting the first selected session chooses the next visible session
- Deleting the last selected session chooses the previous visible session
- Deleting the only remaining session falls back to landing
- Late `list_sessions` refresh should not override the already chosen adjacent session

## Validation

- App test for deleting the first selected session focuses the next session
- App test for deleting the last selected session focuses the previous session
- Existing session selection and landing tests still pass
