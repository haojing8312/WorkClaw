# WorkClaw Tool Defer Loading And Selection Design

## Goal

Make WorkClaw's tool platform behave more like `close-code` by separating:

- the full tool pool the runtime is allowed to use
- the smaller recommended tool set initially exposed to the model
- the expansion path when the first recommendation set is insufficient

This design is intentionally internal-only. It does not add user-facing settings, mode pickers, or a standalone diagnostics UI.

## Current State

WorkClaw already has most of the platform foundation:

- unified `effective_tool_set`
- tool metadata, source/category typing, and policy filters
- session/runtime/skill policy inputs
- structured `EffectiveToolDecisionRecord`
- internal tool discovery and per-task candidate recommendation
- structured route/journal/observability recording

What is still missing is the core loading behavior. Today the runtime still mostly computes a large allowed tool set and exposes it as a whole. Discovery exists, but it is mainly used for prompt guidance and internal recording.

## Problem

This creates three gaps versus the desired architecture:

1. The model sees too much tool surface too early.
2. Recommendation does not yet materially control initial tool exposure.
3. Fallback behavior is not modeled as an explicit expansion from a small set to a larger set.

That means WorkClaw is already well-organized, but not yet truly "assembled on demand".

## Design Principles

1. Preserve correctness and permissions.
   Deferred exposure must never widen actual permissions. It may only narrow or stage exposure.

2. Keep the full tool plan as the source of truth.
   We should not invent a second policy system for recommended tools.

3. Make recommendation reversible.
   If the initial recommended set is insufficient, the runtime must be able to expand safely.

4. Avoid user-facing complexity.
   This remains an internal planner/runtime capability.

5. Keep rollout conservative.
   The first version should bias toward safety and compatibility over aggressive pruning.

## Proposed Architecture

### 1. Split Tool Exposure Into Three Layers

Add three explicit concepts to the runtime tool planner:

- `full_allowed_tools`
  The full policy-validated tool pool the session is allowed to use.

- `recommended_tools`
  The smaller tool subset recommended for the current task based on discovery and tool scoring.

- `active_tools`
  The tool set actually exposed for the current model attempt.

Version 1 behavior:

- `full_allowed_tools` comes from `effective_tool_set`
- `recommended_tools` comes from the current `tool_catalog` ranking
- `active_tools` starts as `recommended_tools` when that set is non-empty and sufficiently safe
- otherwise `active_tools` falls back to `full_allowed_tools`

### 2. Introduce A Deferred Exposure Policy

Add a small internal policy layer that decides whether initial exposure should be:

- `full`
- `recommended_only`
- `recommended_plus_core_safe_tools`

The default recommended behavior should be `recommended_plus_core_safe_tools`.

That means the model still gets:

- task-relevant recommended tools
- a small compatibility floor of core safe tools like file reads / listing / simple inspection when appropriate

This reduces the chance of a hard failure caused by over-pruning.

### 3. Add Expansion Triggers

If the initial active tool set is too narrow, the runtime should expand in a controlled way.

The first version should support these triggers:

- no tool call after an attempt that strongly suggests tool use was needed
- repeated tool-not-available errors caused by deferred exposure
- explicit planner/runtime fallback when the selected skill or attempt reports insufficient tools

The first expansion should go from:

- `recommended_plus_core_safe_tools`
to
- `full_allowed_tools`

Version 1 does not need multi-stage expansion ladders beyond that.

### 4. Promote Recommendation To A Stable Planner Output

The current discovery candidates should be elevated from "extra record data" into a planner artifact with stable semantics:

- which tools were recommended
- why they were recommended
- which fields matched
- whether they were initially active or deferred

This planner artifact should be available to:

- prompt assembly
- route/journal/observability
- fallback explanation
- future tool search

### 5. Keep Route Explanation Adjacent But Separate

Route selection should not be rewritten to directly follow tool recommendation scores.

Instead, route records should gain a structured explanation layer such as:

- tool-side recommendation summary
- whether route selection aligned with that recommendation
- if fallback happened, whether it was due to ambiguous skill recall or insufficient initial tool exposure

This keeps the skill-routing system and tool-selection system separate, but explainable together.

## Data Model Changes

### Planner Output

Extend the current decision/planning output with:

- `full_allowed_tools`
- `recommended_tools`
- `active_tools`
- `deferred_tools`
- `loading_policy`
- `expansion_state`

The existing `EffectiveToolDecisionRecord` should remain the externally serialized record, but it can embed the richer internal planner state or a compact summary of it.

### Recommendation Record

Extend the current discovery candidate records with stable interpretation:

- `match_score`
- `matched_terms`
- `matched_fields`
- `initially_active`
- `deferred_reason`

### Attempt Context

Execution attempts should know:

- what the active tool set was
- whether the run was in deferred mode
- whether expansion has already happened

This is needed for deterministic fallback and observability.

## Runtime Flow

### Initial Attempt

1. Build `effective_tool_set`
2. Build ranked tool recommendations from user message + tool manifest
3. Choose loading policy
4. Derive `active_tools`
5. Assemble prompt with:
   - full policy context only as planner state
   - active tools as the tool list given to the model
   - compact discovery hints based on recommended tools

### Expansion Attempt

1. Detect expansion trigger
2. Mark planner state as expanded
3. Replace `active_tools` with `full_allowed_tools`
4. Re-run attempt with updated tool exposure
5. Record expansion reason in journal/observability

## Compatibility Strategy

The rollout should be additive and reversible.

Phase 1 compatibility rules:

- if recommendation returns empty, use full set
- if recommended set is too small, use recommended plus safe floor
- if expansion fails or planner signal is ambiguous, use full set
- do not change policy enforcement or approval rules

This ensures the feature mostly changes prompt exposure strategy, not runtime authority.

## Testing Strategy

Minimum required tests:

1. Planner tests
   - recommended-only selection
   - recommended-plus-core-safe selection
   - full fallback when no recommendation exists

2. Runtime tool setup tests
   - `active_tools` differs from `full_allowed_tools`
   - deferred tools remain present in planner record but absent from initial exposure

3. Expansion tests
   - tool-not-available or deferred-trigger causes expansion to full set

4. Observability tests
   - decision record includes loading policy and deferred/recommended state

5. Skill config / policy regression tests
   - deferred loading must not bypass source/category/tool deny rules

## Non-Goals

This design explicitly does not do the following:

- user-facing tool configuration
- new settings UI
- separate tool center or diagnostics panel
- route algorithm rewrite based on tool recommendation scores
- fully general tool search UX

## Recommended Execution Order

1. Introduce internal loading policy and planner state.
2. Make `tool_setup` consume `active_tools` instead of the full set.
3. Add one-step expansion to full set.
4. Record deferred/recommended/expanded state in observability.
5. Add route-side explanation that references planner state.

## Expected Outcome

After this work, WorkClaw should behave much closer to `close-code`:

- tools are still policy-governed by one unified planner
- initial tool exposure is smaller and task-shaped
- the runtime can expand when needed
- recommendation becomes operational, not just descriptive
- route/journal/observability can explain what happened without new user UI

## Execution Note

This design was executed on 2026-04-08 in the current workspace with the following practical shape:

- staged tool loading was added with `full/recommended/active/deferred` state
- initial model exposure now uses `active_tools`
- deferred exposure can expand to full tool exposure on a conservative retry
- decision records now carry loading policy, active/deferred counts, and discovery candidates
- route records now include a compact tool-side recommendation summary

Verification used the reliable commands available on this machine:

- `cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
- `pnpm test:rust-fast`

Full runtime Rust test-binary execution was not used as the completion gate because this machine has previously shown a Windows environment issue with `STATUS_ENTRYPOINT_NOT_FOUND` when launching some runtime test binaries.
