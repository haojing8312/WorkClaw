# Skill Creator Refresh Design

**Goal:** Upgrade WorkClaw's built-in skill creation guidance from a static scaffold into a practical authoring workflow that improves trigger quality and introduces lightweight evaluation.

## Scope

This change updates the built-in skill authoring guidance and local template only. It does not introduce a new runtime evaluator, change skill loading semantics, or add new frontmatter parsing.

## Problem

The current built-in skill creator focuses on lightweight structure and basic checklist hygiene. That is enough to generate a valid `SKILL.md`, but not enough to consistently generate discoverable, high-signal skills.

Current gaps:
- No explicit distinction between trigger examples and non-trigger examples
- No iteration loop around false positives / false negatives
- No guidance for choosing among multiple body structures
- No mention of optional advanced frontmatter already supported by runtime

## Design

### Built-in `skill-creator`

Expand the built-in authoring skill from a short checklist into a concise workflow that covers:
- reuse check before creating a new skill
- trigger examples and anti-examples
- description optimization for discovery
- optional advanced frontmatter when needed
- lightweight evaluation with positive and negative prompts

This remains concise and product-oriented. It should not duplicate Anthropic's full reference guide.

### Built-in `skill-creator-guide`

Update the internal local-skill generation guide so the product's scaffold rules match the refreshed authoring guidance:
- keep SKILL concise
- choose structure based on task type
- include positive and negative trigger examples in the default body
- require a lightweight evaluation checklist
- note optional advanced frontmatter fields supported by WorkClaw

### Local template

Replace the current generic template with a more targeted one that includes:
- `Overview`
- `When to Use`
- `When Not to Use`
- `Suggested Structure`
- `Workflow`
- `Prompt Examples`
- `Quality Checklist`

The template should stay lightweight and still be editable by end users.

## Non-goals

- No automatic benchmark runner in this change
- No UI flow redesign
- No migration of existing user-created skills

## Testing

Add content assertions in Rust tests against builtin markdown/template strings. These tests should prove:
- built-in skill creator mentions trigger examples, anti-examples, and evaluation
- guide mentions optional advanced frontmatter and prompt-quality checks
- template contains sections for non-trigger examples and prompt examples
