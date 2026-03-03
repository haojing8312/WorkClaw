# High-Risk Flow UX Standardization Design (SkillMint Runtime)

- Date: 2026-03-03
- Scope: Runtime frontend only
- Goal: Standardize high-risk action UX across runtime views with unified risk levels, confirmation behavior, and visible feedback, without changing backend command semantics.

## 1. Problem Statement

Current high-risk actions are inconsistent across views:

- Some actions execute immediately without a consistent confirmation policy.
- Some actions show loading/feedback, others only log errors.
- Danger severity is not visually or behaviorally standardized.
- Permission risk (`unrestricted`) has weak UX guardrails.

This increases accidental operations and lowers user trust.

## 2. Approach (Selected Option)

Use a **frontend risk policy layer** (selected option 2) with:

1. Unified risk metadata (`low`, `medium`, `high`)
2. Shared confirmation dialog for medium/high actions
3. Unified danger visuals and interaction states
4. Mandatory in-UI success/failure feedback
5. No backend command/permission protocol changes in this iteration

## 3. Risk Taxonomy and UX Rules

### 3.1 Risk Levels

- `low`: No confirmation required; still show loading and completion feedback
- `medium`: Standard confirm dialog required
- `high`: Strong confirm dialog required with explicit impact and irreversibility wording

### 3.2 Required Interaction Contract

For all medium/high actions:

1. Trigger action
2. Open standardized `RiskConfirmDialog`
3. If canceled: do nothing (invoke must not run)
4. If confirmed: run action once, lock duplicate click
5. Show success/failure result in visible UI message area

For all actions:

- Disable repeat submit while in-flight
- Keep button state and wording consistent (`处理中...`)

## 4. Coverage Scope (Full)

All high-risk flows in these areas:

- `Sidebar`: permission mode switching, especially `unrestricted`
- `ChatView`: tool permission confirm, install confirm, stop action consistency
- `ExpertsView`, `FindSkillsView`, `SkillLibraryView`: install/update/remove flows
- `InstallDialog`: install confirmation and failure visibility
- `SettingsView`: delete provider, delete employee, destructive config operations
- `EmployeeHubView`: delete employee
- `PackForm` + `IndustryPackView`: export actions with consistent feedback/locking

## 5. Implementation Design

### 5.1 Shared Layer

Add:

- `apps/runtime/src/components/risk-action.ts`
  - `RiskLevel` type
  - action metadata schema
  - default labels/messages by risk level
- `apps/runtime/src/components/RiskConfirmDialog.tsx`
  - unified dialog layout
  - supports medium/high modes
  - receives title, summary, impact, irreversible note
  - confirm/cancel callbacks + loading lock

### 5.2 Page Integration

Views integrate by passing action metadata and execution callback through a consistent adapter:

- classify operation risk
- open dialog when required
- execute callback only after confirmation
- render feedback in existing message/status sections

No changes to Tauri command names or backend payload formats.

## 6. Accessibility and UX Quality Requirements

Aligned with `ui-ux-pro-max` priorities:

- visible focus rings on all interactive controls
- clear contrast for warning/error text
- no layout shift on hover/active transitions
- explicit error proximity (message close to action area)
- high-risk action text must include irreversible implication

## 7. Risks and Mitigations

Risk: Breaking existing behavior in large components (`ChatView`, `SettingsView`)
- Mitigation: TDD contract tests first, then minimal integration changes

Risk: Inconsistent adoption across all views
- Mitigation: enforce centralized `risk-action` metadata and dialog usage

Risk: Duplicate dialogs and state races
- Mitigation: single active confirmation state per view and in-flight button locks

## 8. Validation Strategy

Automated:

- Extend existing tests and add new risk-flow tests to verify:
  - dialog appears for medium/high
  - cancel prevents invoke
  - confirm triggers invoke once
  - loading prevents duplicate action
  - success/failure feedback visible

Manual:

- verify `unrestricted` switch needs explicit high-risk confirmation
- verify remove/delete operations show impact text
- verify consistent danger styling and wording across all covered views

## 9. Acceptance Criteria

- Every covered destructive/sensitive action is mapped to risk metadata
- Medium/high actions are blocked behind standardized confirmation
- No invoke call occurs on cancel
- Duplicate clicks are prevented during execution
- User-visible success/failure feedback is present for all covered actions
- Existing and new tests pass; runtime build passes

## 10. Rollback Plan

- Rollback order:
  1. page integration changes
  2. shared risk layer wiring
  3. shared risk components (last)
- Keep backend untouched, so rollback is frontend-only and low-risk.
