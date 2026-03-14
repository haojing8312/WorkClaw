# Sidebar Selected Session Scroll Design

**Date:** 2026-03-14

**Goal:** Keep the selected session visible in the sidebar by automatically scrolling it into view whenever selection changes, including restored and auto-focused sessions.

## Scope

- Only affects the session list in the expanded sidebar
- Scroll the selected session item into view when `selectedSessionId` changes
- Also work when selection is restored on startup or changes automatically after delete

## Non-Goals

- No keyboard shortcuts
- No session ordering changes
- No virtualized list behavior
- No change to session data or routing logic

## Approach

Store DOM refs for rendered session rows and run a small effect that calls `scrollIntoView({ block: "nearest", inline: "nearest" })` for the currently selected row. Using `nearest` keeps movement subtle and avoids yanking the list to the top or center.

This keeps the behavior aligned with the lightweight tab metaphor:

- the active session remains visible
- manual and automatic selection changes behave the same
- no new UI or settings are introduced

## Edge Cases

- No-op when there is no selected session
- No-op when the sidebar is collapsed
- No-op when the selected session is not currently rendered
- Re-running on session list refresh is acceptable because `nearest` minimizes unnecessary movement

## Validation

- Sidebar test that selection change triggers `scrollIntoView` for the selected session row
- Existing sidebar tests remain green
- TypeScript remains green
