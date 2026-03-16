# Provider Default Model Refresh Design

**Date:** 2026-03-16
**Status:** Approved

## Goal

Refresh the built-in provider presets used by Settings model connection and first-run quick setup so selecting a provider auto-fills a current agent-capable recommended model instead of stale lightweight defaults.

## Scope

- Update the shared frontend provider catalog in `apps/runtime/src/model-provider-catalog.ts`
- Keep the existing provider IDs, base URLs, and quick-setup flow unchanged
- Reorder preset model suggestion lists so the strongest recommended default appears first
- Update tests that assert provider defaults in quick setup and catalog behavior

## Decision

Use the shared static provider catalog as the single source of truth for:

- settings model preset autofill
- first-run quick model setup autofill
- model suggestion ordering

This is intentionally static rather than runtime-fetched because provider recommendation APIs are inconsistent across vendors and would add startup/network failure modes to onboarding.

## Default Model Policy

- Prefer current official or vendor-recommended agent/coding/tool-use capable models
- Prefer stable model IDs over ephemeral preview IDs when official guidance is ambiguous
- If a vendor has a clear OpenClaw / agent recommendation, use that instead of an older generic chat default
- Preserve older model IDs in the selectable list when they may still be useful

## Initial Refresh Targets

- Zhipu GLM: `glm-5-turbo`
- OpenAI: `gpt-5.4`
- Anthropic: move away from `claude-3-5-haiku-20241022` to the strongest current listed Claude preset
- DeepSeek: keep `deepseek-chat` because it supports tool-calling better than reasoning-only options
- Qwen CN / Intl: `qwen3.5-plus`

## Out of Scope

- Dynamic provider model discovery
- Per-capability model routing changes
- Database migrations
- Changing saved user configs retroactively
