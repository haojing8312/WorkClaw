# Runtime Skill Auto-Routing Design

**Date:** 2026-02-27  
**Status:** Approved  
**Scope:** Runtime only

## 1. Product Intent

Build a production-ready Runtime experience for automatic parent-to-child Skill routing, while preserving WorkClaw's existing target users and security posture.

Constraints confirmed:
- Routing mode: automatic only
- Permission model: strict parent inheritance with progressive narrowing
- UX policy: simple default chat, detailed traces in right-side panel

## 2. Goals and Non-Goals

### 2.1 Goals

1. Users complete tasks without manually selecting child skills.
2. Auto-routing is observable and debuggable when needed.
3. Child-skill execution never exceeds parent session permissions.
4. Failures are understandable with actionable remediation.

### 2.2 Non-Goals

1. Studio-side authoring workflows.
2. Marketplace packaging/version distribution UX.
3. Runtime manual routing mode.

## 3. End-to-End User Flow

1. User sends natural-language request in chat.
2. Runtime decides whether to invoke child skills.
3. Main chat remains clean (assistant answer + short routing summary capsule).
4. Right-side panel receives real-time routing details.
5. If failure occurs, main chat shows human-readable error; panel highlights failing node and suggestion.

## 4. Information Architecture

## 4.1 Main Chat (default simple mode)

Shows only:
- user messages
- assistant responses
- required interaction cards (`ask_user`, permission confirmation)
- one-line routing summary capsule when child skills were invoked

Hides:
- raw technical event logs
- full per-step JSON payloads

## 4.2 Right Panel (enhanced from existing panel)

Tabs:
1. `Overview`: route count, success/failure, total duration, slowest node.
2. `Call Graph`: tree view of parent/child/grandchild skill execution.
3. `Permissions`: parent-to-child narrowing diffs and denial reasons.
4. `Logs` (advanced toggle): raw event stream for diagnostics.

## 5. Interaction Design

## 5.1 Routing Summary Capsule

Assistant message footer capsule:
- "Auto-routed 3 child skills · 2.4s · all succeeded"
- click opens right panel to the matching call graph node

## 5.2 Call Graph Node Model

Each node displays:
- `skillName`
- `depth`
- `status` (`routing`, `executing`, `waiting_user`, `confirm_required`, `completed`, `failed`, `cancelled`)
- `durationMs`
- `mode` (`inline`, reserve for future `fork`)

Default behavior:
- collapse successful branches
- auto-expand failed branches

## 5.3 Error Experience

Main chat:
- concise human message
- one "View details" action

Right panel:
- exact error code
- root cause
- suggested fix

Canonical error classes:
- `SKILL_NOT_FOUND`
- `CALL_DEPTH_EXCEEDED`
- `CALL_CYCLE_DETECTED`
- `PERMISSION_DENIED`
- `TIMEOUT`

## 6. Permission Model

Rule:
`child_allowed = parent_allowed ∩ child_declared_allowed ∩ workspace_policy`

Behavior:
1. Child skill cannot request permission outside parent boundary.
2. No implicit elevation path for child layers.
3. Denials produce blocked node with explainable reason.

UI mapping:
- green: inherited/no narrowing
- yellow: narrowed with reason
- red: blocked/denied

## 7. Runtime Data and Events

## 7.1 Frontend State Additions

Chat/session scope:
- `routeRuns[]`
- `activeRouteRunId`
- `panelTab`
- `selectedRouteNodeId`
- `showAdvancedLogs`

## 7.2 Event Extensions

Add structured events (or normalize existing events):
- `skill-route-started`
- `skill-route-node-updated`
- `skill-route-completed`
- `skill-route-failed`

Each payload includes:
- `session_id`
- `route_run_id`
- `node_id`
- `parent_node_id`
- `skill_name`
- `depth`
- `status`
- `timing`
- `error` (optional)
- `permission_snapshot` (optional)

## 8. Settings

Runtime settings for auto-routing:
1. Max call depth (default `4`, range `2-8`)
2. Node timeout seconds (default `60`)
3. Retry count (default `0`, range `0-2`)
4. Persist right-panel open state

No toggle for manual routing mode in this design.

## 9. Testing and Acceptance

## 9.1 Functional

1. Auto-route success path renders summary capsule + full panel trace.
2. Failure path highlights node and displays remediation.
3. Cycle/depth/not-found/timeout map to stable error UX.
4. Session switch correctly resets transient route state.

## 9.2 Security

1. Child permission set is strict subset/equal to parent.
2. Forbidden tool/path use is blocked and visible in permissions tab.

## 9.3 UX

1. Main chat remains low-noise in 95% of sessions.
2. User reaches failing node details within 2 clicks.

## 10. Delivery Phases

1. Phase A: call graph + summary capsule + overview.
2. Phase B: permissions tab + actionable error panel.
3. Phase C: advanced logs, replay helpers, filter/search in route traces.
