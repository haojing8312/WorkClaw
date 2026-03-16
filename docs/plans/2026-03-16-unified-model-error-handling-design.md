# Unified Model Error Handling Design

**Date:** 2026-03-16
**Status:** Approved

## Goal

Unify user-facing model error handling across model connection testing, quick setup, and chat failure surfaces so that billing, authentication, rate limiting, timeout, and network issues are communicated consistently without provider-specific UI logic.

## Problem

Current model error handling is fragmented:

1. `test_connection_cmd` returns `Result<bool, String>`, so the UI either shows a generic success/failure state or dumps raw backend text.
2. Chat runtime already has a shared error classifier in `chat_policy.rs`, but connection testing does not reuse it.
3. Users can misinterpret upstream billing or quota failures as WorkClaw product defects because some entry points collapse those errors into "连接失败，请检查配置".
4. Supporting more providers increases the risk of duplicated provider-specific conditionals in frontend code.

## Design Principles

- Classify by error semantics, not provider brand.
- Keep one backend classifier as the source of truth.
- Use one frontend presentation model across all entry points.
- Preserve raw upstream errors for diagnostics without making them the primary UI message.
- Prefer minimal schema expansion over deep logging or database migrations.

## Decision

Introduce a shared structured error result for model operations and reuse a single backend classification pipeline across:

- model connection testing
- quick model setup connection testing
- chat failure cards
- future provider health checks

## Unified Error Model

Backend returns a normalized result structure instead of raw booleans for connection testing.

```ts
type ModelOperationResult = {
  ok: boolean;
  kind: "billing" | "auth" | "rate_limit" | "timeout" | "network" | "unknown";
  title: string;
  message: string;
  raw_message?: string;
};
```

### Field semantics

- `ok`: operation success state
- `kind`: stable machine-readable error bucket
- `title`: short human-readable label for compact UI surfaces
- `message`: explanatory guidance text for the user
- `raw_message`: original provider or gateway error text for details, copy, or diagnostics

## Error Classification Strategy

Classification is based on semantic buckets, not provider enumerations.

### Buckets

- `billing`
  - `insufficient_balance`
  - `insufficient balance`
  - `account balance too low`
  - `insufficient_quota`
  - `insufficient quota`
  - `payment required`
  - `credit balance`
  - `余额不足`
  - `欠费`
- `auth`
  - `invalid_api_key`
  - `unauthorized`
  - `authentication`
  - `forbidden`
  - `permission denied`
  - `api key`
- `rate_limit`
  - `rate limit`
  - `too many requests`
  - `429`
  - `quota exceeded`
- `timeout`
  - `timeout`
  - `timed out`
  - `deadline exceeded`
- `network`
  - `connection reset`
  - `dns`
  - `socket`
  - `connect failed`
  - `error sending request for url`
- `unknown`
  - fallback when no semantic bucket matches

### Parsing rules

1. Prefer structured error extraction when a provider returns JSON.
   - OpenAI-compatible responses commonly expose `error.message` and sometimes `error.code`.
   - Anthropic-compatible or gateway responses may still be JSON and should be parsed similarly when possible.
2. Fall back to normalized raw text matching when the upstream response is plain text, mixed gateway text, or unexpected content.
3. Never branch on `provider_key` for user-facing classification unless a future provider introduces a truly incompatible error shape that cannot be normalized with generic parsing.

## Backend Architecture

### Shared classifier

Extract or expand the existing classifier in `apps/runtime/src-tauri/src/commands/chat_policy.rs` into a shared model error normalization utility that can be reused outside chat routing.

Responsibilities:

- derive `kind`
- map `kind` to canonical `title`
- map `kind` to canonical `message`
- preserve `raw_message`

### Connection testing

Change `test_connection_cmd` in `apps/runtime/src-tauri/src/commands/models.rs` from:

- `Result<bool, String>`

to:

- `Result<ModelOperationResult, String>` or equivalent serializable struct

Adapter behavior should be updated so connection tests retain upstream error bodies instead of silently flattening them into booleans.

### Chat runtime

Existing chat failure handling should keep the same semantic buckets but source UI copy from the shared normalized mapping, so connection testing and chat failures stay aligned.

## Frontend Architecture

The frontend should consume the structured backend result directly and avoid reimplementing classification.

### Entry points

- `apps/runtime/src/components/SettingsView.tsx`
- `apps/runtime/src/App.tsx` quick model setup
- `apps/runtime/src/components/ChatView.tsx`

### Display rules

All entry points share the same semantic mapping:

- `billing`
  - title: `模型余额不足`
  - message: `当前模型平台返回余额或额度不足，请到对应服务商控制台充值或检查套餐额度。`
- `auth`
  - title: `鉴权失败`
  - message: `请检查 API Key、组织权限或接口访问范围是否正确。`
- `rate_limit`
  - title: `请求过于频繁`
  - message: `模型平台当前触发限流，请稍后重试或降低并发频率。`
- `timeout`
  - title: `请求超时`
  - message: `模型平台响应超时，请稍后重试，或检查网络和所选模型是否可用。`
- `network`
  - title: `网络连接失败`
  - message: `无法连接到模型接口，请检查 Base URL、网络环境或代理配置。`
- `unknown`
  - title: `连接失败`
  - message: `模型平台返回了未识别错误，可查看详细信息进一步排查。`

### UX guidance

- Primary UI should show `title` and `message`, not raw provider text.
- Raw provider text should remain available behind a detail affordance or copy action.
- Old generic text such as `连接失败，请检查配置后重试` becomes a fallback only for `unknown`.

## Scope

### In scope

- Shared model error normalization in Rust
- Structured connection test result payload
- Shared frontend presentation for:
  - Settings model connection
  - Quick model setup
  - Chat failure card
- Unit tests for classifier and connection test payload handling
- Frontend tests covering billing and auth examples

### Out of scope

- Provider-specific exception tables
- Full health dashboard rewrite
- Historical database migration for old logs
- New i18n framework

## Testing Strategy

### Rust

- classifier unit tests for all six error kinds
- JSON extraction tests for OpenAI-compatible errors
- plain text fallback tests for Anthropic-compatible or proxy errors
- command tests for `test_connection_cmd` structured responses

### Frontend

- settings connection test renders `billing` as `模型余额不足`
- quick setup renders `auth` and `network` using shared text
- chat failure card uses the same `title` and `message` mapping as connection testing

## Risks

- Upstream providers may change error text wording, which could require keyword bucket updates.
- Some gateways return malformed non-JSON content, so fallback text classification must remain robust.
- If frontend keeps any legacy string-based logic, the system can drift back into inconsistent messaging.

## Success Criteria

1. A billing failure from any supported provider is surfaced as `模型余额不足` rather than a generic connection failure.
2. Authentication, rate limit, timeout, and network failures display consistent labels across setup and runtime.
3. Adding a new provider does not require new user-facing error branches when it follows existing semantic error patterns.
