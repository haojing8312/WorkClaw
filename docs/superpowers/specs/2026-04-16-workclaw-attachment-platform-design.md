# WorkClaw Attachment Platform Design

Date: 2026-04-16

## Summary

This design upgrades WorkClaw attachments from a frontend-only file picker feature into a runtime-level attachment platform aligned with the OpenClaw direction.

The platform should support broader attachment capabilities, broader input sources, unified policy enforcement, and adapter-aware runtime semantics. The first implementation plan should cover `P0 + P1`. `P2` is included here as the architecture target, not as the first delivery commitment.

## Problem Statement

WorkClaw currently treats attachments as a thin UI convenience:

- The effective capability surface is limited to `image`, `text-file`, and `pdf-file`.
- Most limits are enforced in frontend code with duplicated constants and divergent behavior between chat and new-session entry flows.
- Only images remain native multimodal inputs by the time the runtime reaches provider adapters.
- Text files and PDFs are flattened into prompt text early, which loses attachment semantics and constrains future provider-native file/media support.
- The current structure is hard to extend to `audio`, `video`, remote URLs, local paths, base64 payloads, or future IM/channel attachment flows.

This creates three strategic gaps relative to OpenClaw:

1. Capability surface is too narrow.
2. Policy and validation are not centralized.
3. Runtime attachment semantics collapse too early.

## Goals

- Align WorkClaw's attachment architecture with the OpenClaw direction.
- Expand the attachment capability model from the current narrow set to `image`, `audio`, `video`, and `document`.
- Support multiple attachment sources, not just browser `File` objects.
- Centralize attachment policy so frontend and backend share one contract.
- Preserve attachment semantics deeper into runtime execution so adapters can choose native ingestion or explicit fallback.
- Make the attachment platform reusable across desktop chat, new-session bootstrap, future tools/skills, and future IM/channel attachment ingestion.

## Non-Goals

- Full parity with every OpenClaw media workflow in the first implementation.
- First-pass implementation of every IM/channel-specific attachment send/receive path.
- Perfect provider-native attachment support across all providers in the first phase.
- Full OpenClaw-style auto-reply media pipeline in the first implementation wave.

## Design Principles

- Preserve attachment meaning until the latest safe stage. Do not flatten early unless a fallback path requires it.
- Keep policy authoritative in backend/runtime code. Frontend validation should be advisory and user-friendly, not the final source of truth.
- Normalize all supported input sources into a single runtime shape before model adaptation.
- Make fallback behavior explicit. Unsupported attachments must be converted intentionally or rejected clearly, never silently ignored.
- Design for reuse across chat, tools, and future channel integrations.

## Target Capability Model

### Attachment kinds

The unified attachment capability model should support:

- `image`
- `audio`
- `video`
- `document`

This intentionally replaces the current protocol bias toward `image`, `file_text`, and `pdf_file` as the primary conceptual model.

### Attachment source types

The attachment input model should support:

- `browser_file`
- `local_path`
- `file_url`
- `remote_url`
- `data_url`
- `base64`

This matches the OpenClaw direction of handling local, remote, and encoded media through one normalized intake layer.

## Data Model

The platform should use three layered models.

### 1. `AttachmentDraft`

`AttachmentDraft` is the frontend-local interaction state. It exists after the user selects, drags, pastes, or enters an attachment source, but before backend validation.

Responsibilities:

- power previews
- hold temporary UI errors and warnings
- support source-specific editing state
- support removal/retry actions

Typical fields:

- `id`
- `kind`
- `source`
- `name`
- `declaredMimeType`
- `sizeBytes`
- `previewUrl`
- `status`
- `warnings[]`
- `error`

### 2. `AttachmentInput`

`AttachmentInput` is the shared frontend/backend contract representing a user-supplied attachment before runtime resolution.

Responsibilities:

- standardize shape across all entry points
- preserve declared metadata
- preserve source semantics
- become the canonical input for validation and runtime resolution

Typical fields:

- `id`
- `kind`
- `sourceType`
- `name`
- `declaredMimeType`
- `sizeBytes`
- `sourcePayload`
- `origin`
- `metadata`

### 3. `AttachmentResolved`

`AttachmentResolved` is the normalized runtime attachment state after validation, MIME resolution, source loading, and fallback preparation.

Responsibilities:

- represent resolved MIME/type facts
- provide safe runtime payloads
- carry truncation/extraction/transcription/summarization results
- support adapter decisions

Typical fields:

- `id`
- `kind`
- `sourceType`
- `name`
- `resolvedMimeType`
- `sizeBytes`
- `sha256`
- `warnings[]`
- `truncated`
- `resolvedPayload`
- `extractedText`
- `transcript`
- `derivedFrames`
- `summary`

## Policy Layer

Current frontend constants such as max file count and type-specific size limits should be replaced by a unified `AttachmentPolicy` structure.

### Policy shape

There should be:

- a global attachment policy
- capability-specific policies for `image`, `audio`, `video`, and `document`

Suggested minimum fields:

- `enabled`
- `maxAttachments`
- `maxBytes`
- `maxChars`
- `mode`
- `prefer`
- `allowedMimeTypes`
- `allowedExtensions`
- `allowSources`
- `fallbackBehavior`

### Selection and fallback semantics

The structure should align with the OpenClaw direction:

- `mode: first | all`
- `prefer: first | last | path | url`

Fallback behavior must be explicit:

- `native`
- `extract_text`
- `transcribe`
- `summarize`
- `reject`

### Authority rules

Frontend code may mirror policy for user-friendly pre-checks, but backend/runtime code must be the authoritative enforcement point.

This prevents policy bypass from:

- future IM/channel entry points
- plugin/tool-driven attachment inputs
- future external APIs

## Runtime Pipeline

The attachment platform should be structured as a five-stage pipeline.

### 1. Collect

Entry points collect attachment candidates and create `AttachmentDraft` objects.

Supported future sources include:

- file picker
- drag and drop
- paste
- local path entry
- URL entry

This stage is interaction-focused only.

### 2. Normalize

The shared normalizer converts heterogeneous sources into `AttachmentInput`.

Responsibilities:

- standardize source representation
- derive or repair names
- estimate size before expensive decoding where possible
- perform MIME sniffing with higher priority than user-declared MIME when evidence conflicts
- emit structured warnings instead of raw UI-only alerts

### 3. Validate

Backend/runtime validation checks the normalized input against `AttachmentPolicy`.

Responsibilities:

- enforce authoritative limits
- reject unsupported source-type and capability combinations
- reject oversized inputs
- reject disallowed MIME/extension combinations
- produce structured validation errors

### 4. Resolve

Runtime resolution converts accepted `AttachmentInput` into `AttachmentResolved`.

Responsibilities:

- load bytes or references safely
- finalize resolved MIME
- prepare provider-native payloads where available
- prepare fallback artifacts when native ingestion is unavailable
- support future offload/reference-style attachment handling

Examples:

- `document` may gain `extractedText`
- `audio` may gain `transcript`
- `video` may gain `derivedFrames` or `summary`
- large payloads may later become managed attachment references instead of inline message body data

### 5. Adapt

Provider adapters consume `AttachmentResolved` and explicitly choose one of:

- native attachment ingestion
- structured fallback transformation
- explicit rejection

Adapters must expose an attachment support matrix. Unsupported attachment kinds must never be silently dropped.

## Provider Semantics

### Current state to replace

Today, WorkClaw effectively preserves only image multimodal semantics. Text files and PDFs are flattened into transcript text before provider adaptation.

### Target state

The target state is:

- attachment semantics survive past transcript assembly
- adapters choose the final representation
- transcript fallback happens only when required by the adapter/provider path

This enables future support for:

- OpenAI-native file/media inputs where available
- audio transcription first, then text fallback
- video frame extraction or summarization fallback
- richer document handling beyond current PDF/text flattening

## Surface-Area Changes

### Frontend

The following frontend changes are expected:

- replace narrow attachment draft types with the new layered model
- unify chat-composer and new-session attachment logic
- remove policy duplication across chat and new-session entry flows
- replace `alert`-only failures with structured inline or toast-style messaging
- add source-aware inputs such as URL/path entry in later phases
- ensure `accept` reflects current policy

### Tauri/backend

The following Tauri/runtime changes are expected:

- replace narrow `SendMessagePart` assumptions with attachment-platform-aware structures
- add centralized validation in backend code
- add runtime normalization and resolution modules
- preserve compatibility when reconstructing existing stored message content
- defer flattening until adapter/fallback decisions

### Adapter layer

The following adapter changes are expected:

- declare supported attachment capabilities
- support native `input_image` and future `input_file`-style flows where possible
- make fallback choices explicit
- reject unsupported combinations with clear errors

## Compatibility Requirements

This work changes user-visible runtime behavior, so compatibility must be handled deliberately.

### Required compatibility constraints

- existing stored sessions must continue to load
- current image upload behavior must remain stable
- existing `contentParts` persistence must keep working during migration
- mixed old/new message content must be reconstructable
- current non-attachment chat behavior must remain unchanged

### Migration posture

The platform should introduce new internal models first while maintaining compatibility shims for legacy message-part shapes until the full migration is complete.

## Phased Delivery

## P0: Foundation and Contract Unification

Goal:

Create the shared attachment platform skeleton and eliminate current rule divergence.

Scope:

- introduce the layered attachment model
- introduce shared attachment policy structures
- unify chat and new-session attachment normalization and pre-check behavior
- add authoritative backend validation
- fix current regressions and UX gaps:
  - divergent text-file handling between chat and new-session flows
  - team-entry attachment loss
  - missing `accept`
  - alert-only rejection behavior

Expected outcome:

The architecture is ready for broader capability expansion even if the effective supported capability set is still limited in practice.

## P1: Multi-Source and Multi-Media Expansion

Goal:

Expand the attachment platform to match the OpenClaw direction for richer media input.

Scope:

- add `audio`, `video`, and generalized `document`
- add source support for:
  - `local_path`
  - `file_url`
  - `remote_url`
  - `data_url`
  - `base64`
- add MIME sniffing and size estimation before full decoding where possible
- define standardized fallback preparation:
  - document text extraction
  - audio transcription
  - video summary or frame-extraction entry points
- add adapter support matrix behavior

Expected outcome:

WorkClaw begins to behave like an attachment platform rather than a file picker.

## P2: Advanced Runtime Parity Direction

Goal:

Reach the intended architecture target for managed media handling and cross-surface reuse.

Scope:

- provider-native file/media ingestion where available
- managed attachment references rather than always-inline payloads
- OpenClaw-like controlled media reference semantics
- tool-output media and chat-upload media convergence
- future IM/channel attachment platform reuse
- scope-aware attachment policy by skill, channel, or chat type

Expected outcome:

The runtime has a true media pipeline rather than isolated attachment features.

## Testing Strategy

The change requires four layers of verification.

### 1. Frontend normalization and UX tests

Must cover:

- shared normalization across chat and new-session entry
- consistent policy-driven acceptance and rejection
- team-entry attachment preservation
- structured user-facing error rendering
- source-aware interaction flows

### 2. Tauri protocol and validation tests

Must cover:

- attachment input deserialization
- authoritative validation behavior
- over-limit and invalid-source rejection
- compatibility with existing stored content

### 3. Transcript and adapter tests

Must cover:

- image native multimodal preservation
- document, audio, and video fallback behavior
- explicit adapter rejection behavior
- no silent attachment loss

### 4. End-to-end regression tests

Must cover:

- new-session attachment bootstrap into first send
- direct chat upload and send
- mixed attachment ordering
- consistent failure behavior for invalid and oversized inputs
- source-type happy-path and reject-path coverage for path/URL flows once enabled

## Acceptance Criteria

The first implementation plan should satisfy all of the following:

- the same attachment behaves consistently whether it enters through new-session or in-chat upload
- backend validation is the source of truth
- adapter behavior is explicit for supported and unsupported attachment kinds
- user-visible rejection, truncation, and downgrade behavior is understandable
- legacy sessions continue to load without regression

## Risks

### Main technical risks

- transcript and adapter changes may break legacy message reconstruction if migration boundaries are not explicit
- policy unification may expose existing hidden inconsistencies as new visible rejects
- file/message protocol expansion may require coordinated frontend, Tauri, persistence, and adapter changes
- different units today mean different things in different places, especially bytes versus text-character truncation

### Main product risks

- broadening capability claims before adapter support is real could create false-positive UX
- adding new source types without strong validation could expand security exposure

## Security and Safety Considerations

The platform must treat attachment sources as untrusted input.

Required design posture:

- MIME sniffing must not trust declarations blindly
- source-type handling must stay explicit and policy-driven
- local-path and URL source handling must be validation-gated
- large-payload handling must avoid unnecessary eager decoding
- future managed-reference work must preserve safe-path and allowlist semantics

## Recommended First Implementation Scope

The first engineering plan should target `P0 + P1`.

That gives WorkClaw a real attachment platform foundation and materially expands capability toward OpenClaw without turning the first delivery into an uncontrolled full-runtime rewrite.
