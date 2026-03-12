# Agent-Reach Content Provider Design

**Date:** 2026-03-11

**Status:** Approved for planning

## Goal

Give WorkClaw a loosely coupled way to use Agent-Reach for content reading and search tasks without vendoring Agent-Reach code, embedding its runtime, or turning it into a hard dependency.

## Scope

Phase 1 covers only content retrieval workflows:

- `read_url`
- `search_content`
- `extract_media_context`

Phase 1 explicitly excludes:

- browser interaction automation
- login-state workflows
- click/type/fill/submit flows
- publish/post/send actions
- multi-step web navigation agents

## Product Positioning

WorkClaw should treat Agent-Reach as an optional external content provider. Users install Agent-Reach themselves. WorkClaw detects whether it is available, surfaces diagnostics, routes suitable tasks to it, and normalizes results for the rest of the runtime.

This keeps the boundary aligned with Agent-Reach's stated model as scaffolding around upstream tools rather than a framework runtime to be embedded.

## Recommended Architecture

### 1. External Content Provider Registry

Add a thin registry for content-capable providers inside the runtime. The registry should:

- list registered providers
- expose provider availability
- expose provider capabilities
- choose the best provider for a high-level task
- support fallback if the preferred provider is unavailable or fails

Initial providers:

- `builtin-web`
- `agent-reach`

### 2. Agent-Reach Adapter

Implement a narrow adapter for Agent-Reach with only four responsibilities:

- detect whether the external command exists
- run diagnostics and summarize availability
- invoke configured read/search/media commands
- normalize stdout/stderr into structured WorkClaw results

The adapter must not vendor Agent-Reach code and must not assume deep internals beyond documented CLI behavior or explicitly configured command templates.

### 3. High-Level Content Tools

Expose only stable, provider-agnostic tools to the agent layer:

- `read_url`
- `search_content`
- `extract_media_context`

These tools route through the registry. The agent should not need to know whether a request is fulfilled by the built-in provider or Agent-Reach.

### 4. Settings and Diagnostics UI

Add an `External Content Providers` section in the runtime UI that shows:

- provider status: `Available`, `Partial`, `Not Found`
- supported capability tags
- last diagnostic result
- setup guide link
- manual `Run Diagnostics` action

## Routing Rules

Use simple rules in phase 1:

- URL reading tasks prefer `agent-reach`
- cross-platform content search tasks prefer `agent-reach`
- media/video context extraction prefers `agent-reach`
- ordinary page interaction tasks remain on existing `browser_*` flows
- write or submit actions never route to `agent-reach`
- if `agent-reach` is unavailable or errors, fall back to `builtin-web` where possible

## Result Contract

All providers should normalize into a shared response shape close to:

```json
{
  "source_provider": "agent-reach",
  "capability": "read_url",
  "title": "Example title",
  "url": "https://example.com",
  "text": "plain text body",
  "markdown": "# Example title",
  "metadata": {
    "platform": "youtube",
    "published_at": "2026-03-01T00:00:00Z"
  },
  "artifacts": []
}
```

The exact type can evolve, but the runtime should standardize provider output before it reaches chat/tool consumers.

## Failure Model

Failures must be productized:

- missing command should not surface raw `ENOENT`
- partial setup should identify missing dependencies
- provider invocation errors should carry actionable diagnostics
- the runtime should state whether fallback was attempted

Example user-facing outcome:

- `Agent-Reach not detected. This task can use an external content provider. WorkClaw fell back to the built-in web reader.`

## Why This Approach

This design preserves loose coupling while still making Agent-Reach feel like a first-class capability source inside WorkClaw.

Compared with direct platform-specific tool integration, it keeps WorkClaw focused on routing and normalization instead of owning every upstream content scraper contract.

Compared with a skill-only integration, it produces a better user experience because users can issue normal reading/search requests and let the runtime choose the provider automatically.

## Risks

### 1. Interface stability

Agent-Reach may not expose a stable machine-oriented API. The adapter should therefore rely on shallow command probing and configurable invocation templates instead of brittle parsing of undocumented output.

### 2. Capability ambiguity

Detection may show Agent-Reach installed while some upstream tools are missing. The diagnostics model must distinguish full, partial, and unavailable states.

### 3. Product confusion

If the UI mixes browser automation with content retrieval, users may assume Agent-Reach supports click/submit workflows through WorkClaw. The UI and docs must keep those capabilities separate.

## Phase 1 Implementation Order

1. Add provider registry and provider status model
2. Add Agent-Reach detection and diagnostics
3. Integrate `read_url`
4. Integrate `search_content`
5. Integrate `extract_media_context`
6. Add UI status and setup guidance
7. Add regression tests and docs

## Non-Goals

- auto-installing Agent-Reach
- modifying user PATH automatically
- vendoring upstream tools
- replacing current Playwright browser automation
- creating a generic browser control layer on top of Agent-Reach
