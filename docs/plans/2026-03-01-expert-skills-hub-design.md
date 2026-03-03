# 2026-03-01 Expert Skills Hub Design

## 1. Background

Phase 1 and 2 completed the task landing experience for normal users. Phase 3 introduces an "Expert Skills" area inspired by MiniMax flow:
- from main app navigation, users can enter a skill-centric page
- users can create a new expert skill via guided conversation
- users can package skills from this domain instead of the home page

## 2. Confirmed Product Decisions

- Main nav label changes from `对话` to `开始任务`.
- Add a new top-level nav item `专家技能`.
- Remove standalone `打包` from home/start-task area to keep the homepage clean.
- `专家技能` page shows `我的技能`.
- `技能社区` is hidden for now.
- `我的技能` includes both installed skills and user-created local skills.
- Creation page must be two-column layout:
  - left: guided chat-like creation flow
  - right: real-time preview
- Save target path is selected by user; if not selected, fallback to:
  - `~/.workclaw/skills/`

## 3. IA and Navigation

### 3.1 Main Navigation (Sidebar)

Top-level:
- 开始任务
- 专家技能
- 设置

### 3.2 Route-like View States

Use route-like page states (no external router dependency required in phase 3):
- `/start-task` (existing chat landing + session chat)
- `/experts` (my skills list page)
- `/experts/new` (create expert skill page)

These states can be mirrored in URL hash/history for forward compatibility with full router adoption.

## 4. Expert Skills Page (`/experts`)

### 4.1 Page Sections

- Header: page title + intro
- Tab area:
  - show `我的技能`
  - hide `技能社区`
- Right-side actions:
  - `创建`
  - `技能打包` (moved here from homepage context)
- Content list:
  - render all installed skills and local-created skills in unified cards/list

### 4.2 Card Information

- name
- short description
- source badge (内置 / 本地 / 加密包)
- version

## 5. Expert Skill Creation Page (`/experts/new`)

### 5.1 Two-Column Layout

Left panel:
- guided creation conversation form flow
- asks key inputs:
  - skill name
  - description
  - when to use
  - constraints / boundaries
  - optional examples
- directory selection control

Right panel:
- real-time preview of generated `SKILL.md` draft
- metadata summary (name, path, status)
- save + back actions

### 5.2 Skill Writing Guidance

Prompting and structure should align with:
- official skill-creation concepts
- local `writing-skills` principles (frontmatter, trigger-oriented description, concise sections)

### 5.3 Save Rules

User chooses save directory during flow.

If skipped/cancelled:
- fallback to `~/.workclaw/skills/`.

Persist as:
- `<base-dir>/<slug>/SKILL.md`

After save:
- import/register local skill
- return to `/experts`
- refresh list and show the new item

## 6. Technical Changes

## 6.1 Frontend

- `App.tsx`:
  - add page-state routing for start-task / experts / experts-new
- `Sidebar.tsx`:
  - nav structure update
  - remove standalone packaging entry from home flow
- add components:
  - `components/experts/ExpertsView.tsx`
  - `components/experts/ExpertCreateView.tsx`

## 6.2 Backend (Tauri commands)

Add command(s) in `commands/skills.rs`:
- create local skill file from structured input
- ensure fallback directory exists
- write `SKILL.md`
- return created path

Then call existing local import command (`import_local_skill`) from frontend to register in DB.

## 7. Error Handling

- directory picker cancel => fallback to default path without blocking
- invalid skill name => inline validation
- write failure => show explicit error and keep draft intact
- import failure after write => show partial-success warning with saved path

## 8. Testing Strategy

Frontend tests:
- nav switch: start-task / experts / experts-new
- experts page: my skills visible, community hidden
- create flow:
  - draft preview updates
  - save uses selected path
  - fallback to default path when no selection

Backend tests:
- default directory fallback resolution
- create and write `SKILL.md`
- invalid inputs rejected

Regression:
- existing landing page and chat session tests stay green
- existing build remains successful

