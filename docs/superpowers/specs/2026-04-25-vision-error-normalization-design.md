# Vision Error Normalization And Image Context Design

## Background

WorkClaw recently added image understanding through vision-capable model routes. A test against a vLLM-hosted `Qwen3-VL-32B-Instruct` endpoint failed with a raw OpenAI-compatible error:

```text
max_tokens must be at least 1, got -1024
```

The deployment-side context settings were later increased and the immediate failure disappeared. The remaining product problem is that WorkClaw currently exposes low-level provider errors too directly, making it hard for users to tell whether the issue is image size, history size, model configuration, rate limits, or an unsupported request shape.

This design follows the shared patterns found in the local `references/openclaw` and `close code` references:

- Normalize provider errors before user display.
- Preserve raw details internally for diagnosis and recovery.
- Validate and reduce image payloads before sending them to a model.
- Avoid repeatedly injecting old image base64 payloads into future model requests.

This design intentionally does not add a model settings vision test, a new technical-details UI panel, or a full doctor-style diagnostics surface.

## Goals

1. Show actionable, user-friendly model errors for context overflow, invalid token budget, and media-size failures.
2. Avoid misclassifying `max_tokens must be at least 1, got -N` as a vision-only problem.
3. Preflight image attachments before they become model payloads.
4. Prevent old image payloads from repeatedly consuming model context.
5. Keep the implementation aligned with existing WorkClaw runtime and attachment boundaries.

## Non-Goals

- No model configuration page health check for vision models.
- No new technical-details expand/copy panel.
- No full doctor or diagnostics contribution framework.
- No complex UI for re-referencing old images.
- No provider-specific vLLM settings editor.
- No change to packaging, release lanes, or vendor sync.

## Reference Patterns

OpenClaw classifies raw provider errors before formatting user copy. It treats context overflow as a semantic class, not a single keyword match, and avoids classifying unrelated rate-limit or reasoning errors as context overflow.

Close code uses a similar boundary: API errors become synthetic assistant error messages with stable user copy and raw `errorDetails` for recovery logic. It also validates and compresses images before the API call, and treats old media as a context-management surface.

The WorkClaw design borrows only the overlapping product behavior:

- classify first;
- display short action-oriented copy;
- keep raw detail internal;
- validate media before send;
- stop carrying old image payloads indefinitely.

## Current WorkClaw Surface

The current OpenAI-compatible adapter returns raw provider failures from the non-success HTTP path:

- `apps/runtime/src-tauri/src/adapters/openai.rs`

The current frontend error display already has model error categories for billing, auth, rate limit, timeout, network, and unknown:

- `apps/runtime/src/lib/model-error-display.ts`

The current attachment platform already has policy and validation boundaries:

- `apps/runtime/src/lib/attachmentPolicy.ts`
- `apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs`

The current transcript builder converts image parts into provider payload blocks:

- `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`

## Design

### 1. Provider Error Normalization

Add a small backend normalization layer for model/provider errors. The normalizer accepts raw error text plus optional context such as provider, model name, base URL, HTTP status, and whether the current request contained images.

It returns a stable internal shape:

```rust
struct NormalizedModelError {
    kind: ModelErrorKind,
    user_title: String,
    user_message: String,
    raw_message: String,
    retryable: bool,
}
```

The exact Rust placement can follow the existing adapter structure, for example a small module under the Tauri runtime model/provider area rather than embedding matching logic directly inside `openai.rs`.

Initial categories:

- `context_overflow`: explicit context-window signals such as `prompt too long`, `context length`, `maximum context`, `context window exceeded`, `model_context_window_exceeded`, or Chinese context-overflow phrases.
- `invalid_token_budget`: `max_tokens must be at least 1` or `got -N` without an explicit context-window phrase.
- `media_too_large`: image size, image dimension, payload too large, request too large, or body-size errors.
- Existing categories: billing, auth, rate_limit, timeout, network, unknown.

`invalid_token_budget` should use careful copy:

```text
模型请求没有剩余空间生成回复。请减少当前会话上下文、压缩图片，或使用更大上下文的模型后重试。
```

If the request contained images, the UI copy may include a secondary hint:

```text
本次请求包含图片，图片内容可能占用了较多上下文。
```

It must not say this is definitely a vision-model context issue.

### 2. Frontend Error Display Alignment

Extend the existing model error display categories instead of introducing a new error UI. The frontend should map the new categories to short Chinese copy:

- `context_overflow`: context too large; reduce history, start a new session, or use a larger-context model.
- `invalid_token_budget`: no output budget remains; reduce context or media payload.
- `media_too_large`: uploaded image or request payload is too large; compress or remove attachments.

Raw provider text remains available in existing runtime data or logs, but the normal chat failure display should prefer the normalized title and message.

### 3. Image Attachment Preflight

Keep the existing attachment count and size defaults:

- maximum 3 image files;
- maximum 5 MB per image;
- maximum 10 MB total image payload.

Add a single image-sanitization step before images enter model transcript payloads:

1. Validate the data URL or base64 payload is non-empty.
2. Validate or infer MIME type.
3. Reject unsupported image types with an attachment error.
4. If the image is above the configured image payload threshold and can be reduced safely, resize/compress it before model submission.
5. If it still cannot fit, fail before the provider call with `media_too_large`.

The first implementation can prefer conservative size reduction and clear failure over aggressive quality changes. The goal is to prevent avoidable provider-side 400 errors, not to build a full image-processing product.

### 4. Vision Context Policy

The current user turn image should be sent normally after preflight.

Historical image payloads should not be repeatedly included in future model requests. When reconstructing or trimming model history, old image payloads should become text placeholders, for example:

```text
[历史图片 screen.png 已从模型上下文移除]
```

This keeps the visible transcript intact while preventing old base64 payloads from consuming model context on every turn.

The rule should be intentionally simple for this iteration:

- preserve images from the current user turn;
- replace image payloads from older turns with placeholders;
- do not add UI for selecting or restoring old images.

### 5. Recovery Behavior

For this iteration, recovery is limited:

- normalize and display friendly errors;
- avoid sending obviously oversized media;
- avoid old image payloads in context.

Do not add automatic retry, model failover, doctor actions, or model-page probing in this phase.

## Testing

Add focused tests at the touched boundaries:

1. Error normalization tests:
   - `max_tokens must be at least 1, got -1024` maps to `invalid_token_budget`.
   - `prompt is too long` maps to `context_overflow`.
   - `input length and max_tokens exceed context limit` maps to `context_overflow`.
   - image-size payload errors map to `media_too_large`.
   - rate-limit and billing phrases do not get misclassified as context overflow.

2. Attachment/image tests:
   - unsupported image MIME fails before provider submission.
   - oversize image payload returns `media_too_large`.
   - accepted image payloads preserve the current turn image.

3. Transcript/context tests:
   - current turn image becomes OpenAI-compatible `image_url`.
   - older image messages become text placeholders.
   - text-only conversations are unchanged.

4. Frontend display tests:
   - new categories render short Chinese titles and messages.
   - unknown errors still use the existing fallback.

## Rollout

Ship behind normal runtime behavior with no user-facing setting. The change improves copy and request hygiene but does not introduce a new workflow.

If image resizing needs an additional native dependency, keep that as a separately reviewed implementation decision. The minimum acceptable implementation can start with strict validation and history image removal, then add compression only where the existing runtime stack supports it safely.

## Self-Review Notes

- No model-page health check is included.
- No technical-details UI panel is included.
- The `max_tokens got -N` case is not treated as vision-specific.
- The design is scoped to provider errors, image preflight, and image context policy.
- The design reuses existing WorkClaw attachment and error-display surfaces.
