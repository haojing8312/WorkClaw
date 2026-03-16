# Agent Run Guardrails Design

**Date:** 2026-03-16
**Status:** Approved

## Goal

Replace the current generic `达到最大迭代次数` failure mode with layered run guardrails that stop unproductive runs earlier, expose machine-readable stop reasons, and give users actionable recovery options.

## Problem

Current runtime behavior has three issues:

1. `AgentExecutor` treats `max_iterations` as the primary stop condition and emits a generic `error` state when the limit is reached.
2. Different entry points already use different budgets, but the system does not model them explicitly:
   - main executor default: `50`
   - skill flow default: `10`
   - employee flow default: `8`
   - some adapter paths also return hard-coded `达到最大迭代次数 8`
3. The frontend collapses these cases into `执行异常`, so users cannot distinguish between:
   - a real system failure
   - a task that ran out of budget
   - a task that is looping without progress

This is especially damaging for browser-heavy tasks such as publishing, form filling, and multi-step page operations, where the model can keep acting while the page no longer changes.

## Design Principles

- Progress matters more than raw turn count.
- Counts remain as the final safety fuse, not the primary control surface.
- Stop reasons must be structured end-to-end.
- Recoverable stops are product states, not system exceptions.
- Budget policy should be explicit and consistent across chat, skill, employee, and sub-agent flows.
- Deliver in phases so P0 improves UX quickly without blocking deeper runtime work.

## Decision

Introduce a shared run-guardrail model built from four layers:

1. `BudgetGuard`
2. `ProgressGuard`
3. `RunStopReason`
4. `Recovery UI`

`max_iterations` remains, but only as one input into `BudgetGuard`.

## Current Runtime References

- `apps/runtime/src-tauri/src/agent/executor.rs`
  - default executor budget is `50`
  - max-iteration exhaustion emits `agent-state-event { state: "error" }`
  - then returns `Err(anyhow!("达到最大迭代次数 ..."))`
- `apps/runtime/src-tauri/src/commands/chat_send_message_flow.rs`
  - skill default budget is `10`
- `apps/runtime/src-tauri/src/commands/employee_agents.rs`
  - employee default budget is `8`
- `apps/runtime/src/components/ChatView.tsx`
  - `agentState.state === "error"` is rendered as `执行异常`

## Proposed Runtime Model

### 1. BudgetGuard

`BudgetGuard` owns all hard ceilings that prevent runaway execution.

```rust
enum RunBudgetScope {
    GeneralChat,
    Skill,
    Employee,
    SubAgent,
    BrowserHeavy,
}

struct RunBudgetPolicy {
    max_turns: usize,
    max_time_ms: u64,
    max_session_turns: Option<usize>,
    same_tool_warning_threshold: usize,
    same_tool_stop_threshold: usize,
    no_progress_warning_threshold: usize,
    no_progress_stop_threshold: usize,
}
```

Responsibilities:

- resolve default budgets by scope
- apply optional per-skill overrides within bounded limits
- separate:
  - per-run turn budget
  - per-run time budget
  - per-session turn budget
  - sub-agent nesting and child-count budget

Recommended defaults:

- general chat: `12`
- skill: `16`
- employee: `12`
- sub-agent: `6-8`
- browser-heavy: `24`
- session turn limit: `100`

### 2. ProgressGuard

`ProgressGuard` decides whether a run is still making meaningful progress.

It evaluates lightweight `ProgressFingerprint` snapshots captured once per turn.

```rust
struct ProgressFingerprint {
    tool_name: Option<String>,
    tool_input_hash: Option<String>,
    tool_output_hash: Option<String>,
    page_url: Option<String>,
    page_title: Option<String>,
    facts_hash: Option<String>,
    interactive_elements_hash: Option<String>,
}
```

The first implementation should support three generic detectors:

- repeated identical tool calls
- repeated no-progress outcomes
- ping-pong between two tool patterns

For browser-heavy tasks, add two extra detectors:

- page signature unchanged for N turns
- extracted task facts unchanged for N turns

This borrows the strongest ideas from:

- `openclaw`: no-progress and ping-pong detection before allowing more tool execution
- `gemini-cli`: explicit loop detection service with thresholds and typed loop events

### 3. RunStopReason

Stop conditions must be structured rather than encoded in raw Chinese strings.

```rust
enum RunStopReasonKind {
    GoalReached,
    Cancelled,
    MaxTurns,
    MaxSessionTurns,
    Timeout,
    LoopDetected,
    NoProgress,
    ToolFailureCircuitBreaker,
    ProtocolViolation,
}

struct RunStopReason {
    kind: RunStopReasonKind,
    title: String,
    message: String,
    detail: Option<String>,
}
```

This type should become the source of truth for:

- `agent-state-event`
- persisted `session_runs.error_kind`
- `session_run_events`
- frontend display copy

### 4. Recovery UI

The frontend should no longer present every run stop as `执行异常`.

Instead, it should show one of:

- `任务已完成`
- `任务已取消`
- `任务疑似卡住，已自动停止`
- `任务达到执行步数上限`
- `任务执行超时`

Every stop card should include:

- stop title
- short explanation
- last confirmed progress
- current page or tool context when available
- recent tool actions
- recommended next action

Suggested recovery actions:

- retry current stage
- continue once with loop detection disabled
- hand over to user
- open files / logs / task journey

## Browser-Heavy Task Strategy

Browser-heavy tasks should not rely on raw turn budgets alone.

Introduce a minimal stage-oriented execution model for high-friction publish and form flows:

1. open page
2. fill cover or primary metadata
3. choose style or category
4. fill title
5. fill body
6. preflight check
7. wait for confirmation or publish

Each stage defines:

- expected page facts
- success predicate
- fallback or recovery action

This is not a strict workflow engine in P0. It is a lightweight stage hint model that lets the runtime explain where the task stopped.

## Event And Persistence Model

### Agent event payload

Extend `agent-state-event` with structured stop fields.

```ts
type AgentStateEvent = {
  session_id: string;
  state: "thinking" | "tool_calling" | "finished" | "stopped" | "error";
  detail?: string | null;
  iteration: number;
  stop_reason_kind?: string | null;
  stop_reason_title?: string | null;
  stop_reason_message?: string | null;
};
```

Rules:

- real internal faults keep `state = "error"`
- budget or loop stops use `state = "stopped"`
- successful termination uses `state = "finished"`

### Session run persistence

Reuse the existing `session_runs` and `session_run_events` model.

P0 should not require a heavy schema rewrite. Instead:

- persist `error_kind` as:
  - `max_turns`
  - `max_session_turns`
  - `timeout`
  - `loop_detected`
  - `no_progress`
  - `cancelled`
  - `unknown`
- add `session_run_events` records such as:
  - `run_guard_warning`
  - `run_stopped`
  - `progress_snapshot`

This gives enough observability to power UI summaries and future analytics.

## Frontend UX Rules

### Display mapping

- `thinking` -> `正在分析任务`
- `tool_calling` -> `正在处理步骤`
- `stopped` + `loop_detected` -> `任务疑似卡住，已自动停止`
- `stopped` + `max_turns` -> `任务达到执行步数上限`
- `stopped` + `timeout` -> `任务执行超时`
- `error` -> `执行异常`

### Copy guidance

The main text should describe what happened, not expose runtime internals first.

Preferred:

- `已尝试 16 步，但页面状态连续 6 轮未变化，系统已自动停止以避免空转。`

Avoid:

- `达到最大迭代次数 16`

### Progressive disclosure

Primary card:

- title
- message
- last progress

Expandable detail:

- raw runtime detail
- recent tools
- session run id

## Scope

### P0

- typed stop reasons
- unified budget resolution
- frontend stop-state copy
- persistence of structured stop events

### P1

- progress fingerprints
- repeated-tool and no-progress detection
- browser page signature detection

### P2

- stage hints for browser-heavy tasks
- richer recovery actions
- analytics dashboard for loop/no-progress patterns

## Out Of Scope

- full workflow engine for every skill
- provider-specific model routing changes
- redesign of the entire chat transcript layout
- long-term automated self-healing or autonomous replanning

## Testing Strategy

### Rust

- budget policy resolution tests
- repeated-tool and no-progress detector tests
- executor tests for:
  - max turns stop
  - timeout stop
  - stop reason serialization
- persistence tests for `run_stopped` event payloads

### Frontend

- `ChatView` renders `stopped` states with user-friendly copy
- `error` state remains reserved for real execution failures
- task journey summary includes stop title and last progress when available

## Risks

- Thresholds that are too aggressive can stop legitimate long tasks.
- Browser fingerprints can generate false positives if page state is noisy.
- Mixing legacy string-based checks with new typed stop reasons can reintroduce drift.
- P0 improves UX quickly, but without P1 progress detection some users will still hit hard turn limits.

## Success Criteria

1. Users no longer see budget exhaustion presented as a generic execution exception.
2. Loop- and no-progress-related stops are distinguishable in persisted run data.
3. Main chat, skill, employee, and sub-agent flows use a documented and bounded budget policy.
4. Browser-heavy task complaints about `达到最大迭代次数` drop materially after rollout.
