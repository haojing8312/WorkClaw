---
name: creating-local-skills
description: Use when designing or iterating WorkClaw local skill templates and dialog-based creation flow for end users.
---

# Creating Local Skills

## Overview
This built-in guide defines how WorkClaw generates local skills in a stable, maintainable way.
Use it as the single source of truth for skill scaffold structure and quality checks.

## Core Principles
- Keep SKILL.md concise; include only task-relevant instructions.
- Description must be specific and discovery-friendly, including "Use when ..." trigger conditions.
- Use progressive disclosure: keep overview in SKILL.md and link extra files only when needed.
- Prefer one recommended workflow instead of many equivalent options.
- Ensure output can be verified with explicit quality checks.
- Treat trigger quality as a first-class concern: generated skills should reduce both false positives and false negatives.
- Keep default frontmatter minimal, but allow optional advanced fields such as `allowed_tools`, `context`, `agent`, and `mcp-servers` when the skill needs tighter runtime constraints.

## Template Source
- Primary scaffold template: [templates/LOCAL_SKILL_TEMPLATE.md](templates/LOCAL_SKILL_TEMPLATE.md)
- If creation flow changes, update this guide and the template together.

## Creation Quality Checklist
- Frontmatter includes only `name` and `description`.
- Description is in third person and contains concrete trigger context.
- Body includes `Overview`, `When to Use`, `When Not to Use`, `Workflow`, `Prompt Examples`, and `Quality Checklist`.
- Generated paths use forward slashes in examples and references.
- Default scaffold stays under a lightweight token budget.
- Prompt examples cover both should-trigger and should-not-trigger cases.
- Guide and template leave room for optional advanced frontmatter when needed.

## Iteration Loop
1. Observe real user-created skills and identify weak sections.
2. Update template with minimal targeted changes.
3. Validate generated output readability, trigger quality, and prompt examples.
4. Look for `误触发` and `漏触发`, then tighten the description or examples instead of adding verbose instructions.
5. Keep this guide aligned with the latest template behavior.
