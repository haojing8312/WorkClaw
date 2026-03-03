# Unified Design Language Design (WorkClaw Runtime)

- Date: 2026-03-03
- Scope: `apps/runtime/src/App.tsx`, `apps/runtime/src/components/Sidebar.tsx`, `apps/runtime/src/components/ChatView.tsx`, `apps/runtime/src/components/SettingsView.tsx`, `apps/runtime/src/index.css`
- Goal: Implement a complete theme system to unify visual language and interaction behavior without changing business logic.

## 1. Architecture and Boundaries

### 1.1 Theme System Layers

The UI refactor uses a four-layer structure:

1. Design Tokens (global)
- Define color, typography, spacing, radius, elevation, motion, and focus-ring tokens in `index.css`.
- Tokens become the single source of truth for visual styles.

2. Semantic Primitives (reusable classes)
- Build semantic utility classes over tokens, e.g. `sm-app`, `sm-surface`, `sm-panel`, `sm-input`, `sm-btn-primary`, `sm-badge-success`.
- Component files should use semantic classes, not hardcoded palette classes.

3. Component Variants (lightweight variants)
- Standardize key variants for button/card/badge/input within current codebase.
- No new UI framework introduced; variants are class-based and incremental.

4. Page Composition
- Apply the above to the 4 core views to normalize look and interaction quality.

### 1.2 Hard Boundaries

- Do not alter runtime behavior, event flow, or API invocations.
- Do not rename user-facing flows, command semantics, or data structures.
- Keep current layout/feature behavior intact; this is a style-system migration.
- Dark mode switching is out of scope for this iteration (tokens will reserve expansion path).

## 2. Component Inventory and Page Mapping

### 2.1 Token Categories

Define tokens in `:root`:

- Color:
  - `--sm-bg`, `--sm-surface`, `--sm-surface-muted`
  - `--sm-text`, `--sm-text-muted`, `--sm-border`
  - `--sm-primary`, `--sm-primary-strong`
  - `--sm-success`, `--sm-warn`, `--sm-danger`
- Typography:
  - `--sm-font-sans`, size/line-height scale
- Geometry:
  - `--sm-radius-sm`, `--sm-radius-md`, `--sm-radius-lg`
  - spacing scale `--sm-space-1..8`
  - shadow scale
- Motion and interaction:
  - `--sm-duration-fast`, `--sm-duration-base`, easing token
  - `--sm-focus-ring`

### 2.2 Semantic Primitives

Add reusable semantic classes in `@layer components`:

- Surfaces/layout:
  - `sm-app`, `sm-surface`, `sm-panel`, `sm-divider`
- Typography:
  - `sm-text-primary`, `sm-text-muted`, `sm-text-danger`
- Form controls:
  - `sm-input`, `sm-select`, `sm-textarea`, `sm-field-label`
- Buttons:
  - `sm-btn`, `sm-btn-primary`, `sm-btn-secondary`, `sm-btn-ghost`, `sm-btn-danger`
- Badges/states:
  - `sm-badge-info`, `sm-badge-success`, `sm-badge-warn`, `sm-badge-danger`

### 2.3 Core View Refactor Mapping

1. `App.tsx`
- Replace root-level hardcoded gray text/background classes with `sm-app` and semantic text classes.
- Keep view routing and motion transitions unchanged.

2. `Sidebar.tsx`
- Migrate navigation buttons/session rows/settings action to semantic button variants.
- Migrate search/select controls to semantic form primitives.
- Normalize selected/hover/focus states using tokenized primary color.

3. `ChatView.tsx`
- Migrate message cards, tool confirmation cards, install modal, side panel, and input shell to semantic classes.
- Normalize status chips (running/completed/error) via semantic badges.
- Preserve all chat behavior, streaming, tool events, and side-panel logic.

4. `SettingsView.tsx`
- Migrate tabs, cards, form sections, save/test buttons, and success/error notices to semantic classes.
- Keep all invoke/state logic unchanged.

## 3. Data Flow, Risks, Validation, Rollback

### 3.1 Data/Logic Integrity

- React state and effects remain untouched.
- Tauri invoke/listen command paths remain untouched.
- Only style class composition and CSS layers are modified.

### 3.2 Risks and Controls

Risk 1: Logic damage in large files (`ChatView.tsx`, `SettingsView.tsx`)
- Control: refactor in slices (containers -> controls -> states), then run tests after each slice.

Risk 2: Visual regressions (selected/disabled/error state ambiguity)
- Control: manual checks from UI/UX rule priorities (focus visibility, contrast, hover stability, no layout shift).

Risk 3: Incomplete standardization
- Control: enforce semantic-class usage in target files and reduce direct palette class usage materially.

### 3.3 Verification

Automated:
- Run existing frontend tests in runtime app (`vitest`) and confirm core view tests still pass.

Manual:
- Check navigation/session selected states and keyboard focus visibility.
- Check chat input actions (send/stop/attach) and modal readability.
- Check settings tabs/forms/feedback statuses.
- Check responsive widths (375 / 768 / 1024) without horizontal overflow.

### 3.4 Rollback Strategy

- Change set is intentionally limited to `index.css` + 4 core view files.
- If any page becomes unstable, keep token/primitives layer and roll back only page-level class migration for that page.

## 4. Acceptance Criteria

- Core views adopt a unified semantic theme layer.
- Focus-visible, contrast, and interaction states are consistent across the 4 views.
- No business behavior regressions.
- Existing frontend tests pass after migration.
