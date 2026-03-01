# 2026-03-01 Chat Capability Gallery Design

## 1. Background

Phase 1 introduced a no-session landing page for chat mode. Phase 2 extends this landing page with a capability gallery inspired by MiniMax "explore experts" structure, but adapted to SkillMint's product language and visual system.

The target audience is general users. The UI should not expose internal terms such as "Skill".

## 2. Goal

Add a "common task scenarios" section below the chat landing input so users can quickly understand use cases and start from practical templates.

The gallery should reduce blank-page hesitation and improve first-task conversion without adding complex information architecture.

## 3. Scope

### In Scope

- Add a scenario card gallery below the existing no-session landing section.
- Provide exactly 4 curated cards in phase 2.
- Clicking a card fills the landing input with a template prompt.
- Card click does not auto-create session.
- Users still explicitly click `开始新会话` (or Enter) to create/send.

### Out of Scope

- No category tabs.
- No card search.
- No pagination / "more scenarios" panel.
- No dynamic backend-driven scenario generation.
- No dedicated marketplace route yet.

## 4. Information Architecture

Within `NewSessionLanding`:

1. Hero + intro (existing)
2. Main input + create CTA (existing)
3. Recent sessions (existing)
4. Common task scenarios (new, below recent sessions)

This keeps one-page continuity for first-time users and preserves existing app-level state flow.

## 5. Scenario Content

Use static frontend constants for phase 2:

1. 文件整理助手  
Prompt: `请帮我整理下载目录，把文件按类型分类到子文件夹，并按近30天和更早文件分开。先告诉我你的整理方案。`

2. 本地数据汇总  
Prompt: `我有一批本地文件，请帮我提取关键数据并汇总成简明结论，先说明你会如何处理这些文件。`

3. 浏览器信息采集  
Prompt: `请帮我在浏览器中查找这个主题的最新公开信息，并整理成要点列表，标注来源。`

4. 代码问题排查  
Prompt: `我会提供报错和代码片段，请先定位最可能根因，再给出最小可行修复方案。`

Each card includes:
- title
- short description
- promptTemplate

## 6. Interaction Design

### 6.1 Card Click Behavior

On card click:
- Fill landing textarea with card `promptTemplate`.
- Focus textarea.
- Smooth-scroll to input area.
- Show a small hint below input:
  - `已填入场景示例，你可以继续修改后再开始会话`

### 6.2 Existing Input Handling

- Strategy: overwrite existing input directly (simple and predictable for phase 2).
- No confirmation modal in this phase.

### 6.3 Selection Feedback

- Selected card shows blue accent border/background.
- Selection remains until another card is chosen.

## 7. Visual and Responsive Rules

- Keep existing blue + gray runtime style.
- Card layout:
  - mobile: 1 column
  - desktop: 2 columns
  - wide desktop: 4 columns
- Ensure clear click affordance and adequate touch area.

## 8. Component and State Changes

Only modify:
- `apps/runtime/src/components/NewSessionLanding.tsx`

Add local state:
- `selectedScenarioId: string | null`
- `showFilledHint: boolean`

Add refs:
- input textarea ref (for focus + scroll target)

No app-level state or API changes required.

## 9. Testing Strategy

Extend `NewSessionLanding` tests:

1. renders 4 scenario cards
2. clicking card fills textarea with template
3. clicking card does not call create-session callback
4. filled hint appears after card click
5. create callback triggers only after explicit submit
6. selected card gets active styling marker (can verify via class or aria state)

Regression:
- existing App landing tests remain green
- existing session-create flow tests remain green

## 10. Rollout Notes

Phase 2 intentionally keeps scenario data static for quality control and speed.

Future phase can migrate scenario definitions to local JSON or backend-managed data once content operations requirements become clear.

