# Run Budget Continue Design

**Date:** 2026-03-17
**Status:** Approved

## Goal

Raise the default runtime turn budget so normal long-running tasks are not stopped prematurely, while keeping a hard safety fuse and allowing the user to continue a stopped `max_turns` run in bounded increments.

## Decisions

- Set all default run-budget scopes to `100` turns.
- Keep `max_turns` as a structured `stopped` state rather than a runtime error.
- When the user sends a continuation prompt such as `继续` after a `max_turns` stop, grant an additional `100` turns for the next run.
- Each continuation grants another `100` turns. There is no one-time continuation cap.

## Scope

This change covers:

- shared Rust run-budget defaults
- chat send-message request shape so the frontend can request a continuation budget
- frontend stopped-state UX for `max_turns`
- tests for default budgets and continuation behavior

This change does not alter:

- loop detection thresholds
- no-progress thresholds
- timeout handling
- employee-group continuation flows

## UX Rules

- `max_turns` still renders as a recoverable stopped task, not `执行异常`.
- Users can continue in two ways:
  - type `继续` in the composer
  - click a `继续执行` action shown on the `max_turns` failure card
- Continuation budget is only auto-applied when the latest run for the session stopped because of `max_turns`.

## Backend Rules

- `RunBudgetPolicy` defaults become `100` for all current scopes.
- `SendMessageRequest` accepts an optional `maxIterations`.
- When provided, `maxIterations` overrides the resolved default for that run only.
- Continuation runs reuse the existing `send_message` pipeline and append a fresh user message, but run with `max_iterations = 100`.

## Rationale

- The previous `12`-turn default is too restrictive for browser-heavy and debugging workflows.
- Claude Code and Codex best-practice guidance emphasize guardrails, planning, sandboxing, review, and checkpoints more than small default turn ceilings.
- Keeping a hard cap of `100` still protects against runaway loops and cost spikes, while making the cap a last-resort fuse instead of the primary stop mechanism.
