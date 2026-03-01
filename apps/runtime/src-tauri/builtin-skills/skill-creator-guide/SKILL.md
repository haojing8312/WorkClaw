---
name: creating-local-skills
description: Use when designing or iterating SkillMint local skill templates and dialog-based creation flow for end users.
---

# Creating Local Skills

## Overview
This built-in guide defines how SkillMint generates local skills in a stable, maintainable way.
Use it as the single source of truth for skill scaffold structure and quality checks.

## Core Principles
- Keep SKILL.md concise; include only task-relevant instructions.
- Description must be specific and discovery-friendly, including "Use when ..." trigger conditions.
- Use progressive disclosure: keep overview in SKILL.md and link extra files only when needed.
- Prefer one recommended workflow instead of many equivalent options.
- Ensure output can be verified with explicit quality checks.

## Template Source
- Primary scaffold template: [templates/LOCAL_SKILL_TEMPLATE.md](templates/LOCAL_SKILL_TEMPLATE.md)
- If creation flow changes, update this guide and the template together.

## Creation Quality Checklist
- Frontmatter includes only `name` and `description`.
- Description is in third person and contains concrete trigger context.
- Body includes `Overview`, `When to Use`, `Workflow`, and `Quality Checklist`.
- Generated paths use forward slashes in examples and references.
- Default scaffold stays under a lightweight token budget.

## Iteration Loop
1. Observe real user-created skills and identify weak sections.
2. Update template with minimal targeted changes.
3. Validate generated output readability and trigger quality.
4. Keep this guide aligned with the latest template behavior.
