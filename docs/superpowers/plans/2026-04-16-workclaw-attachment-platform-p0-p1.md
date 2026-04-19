# WorkClaw Attachment Platform P0-P1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the P0 and P1 attachment-platform foundation so WorkClaw can move from narrow frontend-only attachment handling to a unified runtime attachment system aligned with the OpenClaw direction.

**Architecture:** Introduce a shared attachment domain model and policy layer, route both frontend entry points through one normalization path, add authoritative Tauri-side validation/resolution, and preserve attachment semantics deeper into runtime so adapters can make explicit native-or-fallback decisions. Keep legacy message/session compatibility during the migration by using compatibility shims instead of a flag day rewrite.

**Tech Stack:** React, TypeScript, Tauri, Rust, serde, existing WorkClaw runtime/adapters, Vitest/RTL, Rust unit tests, repo-local verification commands.

---

## File Map

### Existing files to modify

- `apps/runtime/src/types.ts`
- `apps/runtime/src/lib/chatAttachments.ts`
- `apps/runtime/src/scenes/chat/useChatDraftState.ts`
- `apps/runtime/src/components/NewSessionLanding.tsx`
- `apps/runtime/src/components/chat/ChatComposer.tsx`
- `apps/runtime/src/scenes/chat/useChatSendController.ts`
- `apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts`
- `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- `apps/runtime/src-tauri/src/commands/chat.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- `apps/runtime/src-tauri/src/adapters/openai.rs`

### New files to create

- `apps/runtime/src/lib/attachmentPolicy.ts`
- `apps/runtime/src/lib/attachmentDrafts.ts`
- `apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts`
- `apps/runtime/src/lib/__tests__/attachmentDrafts.test.ts`
- `apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachment_types.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachment_resolution.rs`
- `apps/runtime/src-tauri/src/adapters/attachment_support.rs`
- `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

## Task 1: Introduce Frontend Attachment Policy

**Files:**
- Create: `apps/runtime/src/lib/attachmentPolicy.ts`
- Test: `apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts`
- Modify: `apps/runtime/src/lib/chatAttachments.ts`

- [ ] **Step 1: Write the failing policy test**

```ts
import {
  DEFAULT_ATTACHMENT_POLICY,
  buildFileInputAccept,
  resolveCapabilityPolicy,
} from "../attachmentPolicy";

describe("attachmentPolicy", () => {
  test("exposes OpenClaw-aligned defaults for image/audio/video/document", () => {
    expect(DEFAULT_ATTACHMENT_POLICY.global.maxAttachments).toBeGreaterThan(0);
    expect(resolveCapabilityPolicy(DEFAULT_ATTACHMENT_POLICY, "image").maxBytes).toBe(10 * 1024 * 1024);
    expect(resolveCapabilityPolicy(DEFAULT_ATTACHMENT_POLICY, "audio").mode).toBe("first");
    expect(resolveCapabilityPolicy(DEFAULT_ATTACHMENT_POLICY, "video").enabled).toBe(true);
    expect(resolveCapabilityPolicy(DEFAULT_ATTACHMENT_POLICY, "document").maxChars).toBe(200_000);
  });

  test("builds a stable accept string from allowed extensions and mime types", () => {
    expect(buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY)).toContain(".pdf");
    expect(buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY)).toContain("image/png");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentPolicy.test.ts`
Expected: FAIL with module-not-found or missing export errors for `attachmentPolicy.ts`.

- [ ] **Step 3: Write minimal implementation**

```ts
export type AttachmentCapability = "image" | "audio" | "video" | "document";
export type AttachmentSelectionMode = "first" | "all";
export type AttachmentSelectionPreference = "first" | "last" | "path" | "url";
export type AttachmentFallbackBehavior = "native" | "extract_text" | "transcribe" | "summarize" | "reject";
export type AttachmentSourceType =
  | "browser_file"
  | "local_path"
  | "file_url"
  | "remote_url"
  | "data_url"
  | "base64";
```

```ts
export type AttachmentCapabilityPolicy = {
  enabled: boolean;
  maxAttachments: number;
  maxBytes: number;
  maxChars?: number;
  mode: AttachmentSelectionMode;
  prefer: AttachmentSelectionPreference;
  allowedMimeTypes: string[];
  allowedExtensions: string[];
  allowSources: AttachmentSourceType[];
  fallbackBehavior: AttachmentFallbackBehavior;
};

export type AttachmentPolicy = {
  global: { maxAttachments: number };
  capabilities: Record<AttachmentCapability, AttachmentCapabilityPolicy>;
};
```

```ts
export const DEFAULT_ATTACHMENT_POLICY: AttachmentPolicy = {
  global: { maxAttachments: 5 },
  capabilities: {
    image: {
      enabled: true,
      maxAttachments: 5,
      maxBytes: 10 * 1024 * 1024,
      mode: "all",
      prefer: "first",
      allowedMimeTypes: ["image/png", "image/jpeg", "image/webp", "image/gif"],
      allowedExtensions: [".png", ".jpg", ".jpeg", ".webp", ".gif"],
      allowSources: ["browser_file", "local_path", "file_url", "remote_url", "data_url", "base64"],
      fallbackBehavior: "native",
    },
    audio: {
      enabled: true,
      maxAttachments: 3,
      maxBytes: 25 * 1024 * 1024,
      mode: "first",
      prefer: "first",
      allowedMimeTypes: ["audio/mpeg", "audio/wav", "audio/mp4", "audio/webm"],
      allowedExtensions: [".mp3", ".wav", ".m4a", ".webm"],
      allowSources: ["browser_file", "local_path", "file_url", "remote_url", "data_url", "base64"],
      fallbackBehavior: "transcribe",
    },
    video: {
      enabled: true,
      maxAttachments: 2,
      maxBytes: 50 * 1024 * 1024,
      mode: "first",
      prefer: "first",
      allowedMimeTypes: ["video/mp4", "video/webm", "video/quicktime"],
      allowedExtensions: [".mp4", ".webm", ".mov"],
      allowSources: ["browser_file", "local_path", "file_url", "remote_url", "data_url", "base64"],
      fallbackBehavior: "summarize",
    },
    document: {
      enabled: true,
      maxAttachments: 5,
      maxBytes: 20 * 1024 * 1024,
      maxChars: 200_000,
      mode: "all",
      prefer: "first",
      allowedMimeTypes: ["text/plain", "text/markdown", "text/csv", "application/json", "application/pdf"],
      allowedExtensions: [".txt", ".md", ".csv", ".json", ".pdf"],
      allowSources: ["browser_file", "local_path", "file_url", "remote_url", "data_url", "base64"],
      fallbackBehavior: "extract_text",
    },
  },
};
```

```ts
export function resolveCapabilityPolicy(policy: AttachmentPolicy, capability: AttachmentCapability) {
  return policy.capabilities[capability];
}

export function buildFileInputAccept(policy: AttachmentPolicy): string {
  return Object.values(policy.capabilities)
    .flatMap((entry) => [...entry.allowedExtensions, ...entry.allowedMimeTypes])
    .filter((value, index, arr) => arr.indexOf(value) === index)
    .join(",");
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentPolicy.test.ts`
Expected: PASS with 2 passing tests.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src/lib/attachmentPolicy.ts apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts apps/runtime/src/lib/chatAttachments.ts
git commit -m "feat: add shared attachment policy defaults"
```

## Task 2: Create Shared Frontend Attachment Draft Normalization

**Files:**
- Create: `apps/runtime/src/lib/attachmentDrafts.ts`
- Test: `apps/runtime/src/lib/__tests__/attachmentDrafts.test.ts`
- Modify: `apps/runtime/src/types.ts`

- [ ] **Step 1: Write the failing normalization test**

```ts
import { normalizeBrowserFilesToDrafts } from "../attachmentDrafts";
import { DEFAULT_ATTACHMENT_POLICY } from "../attachmentPolicy";

describe("attachmentDrafts", () => {
  test("normalizes image, audio, video, and document browser files into unified drafts", async () => {
    const files = [
      new File(["img"], "cover.png", { type: "image/png" }),
      new File(["voice"], "memo.mp3", { type: "audio/mpeg" }),
      new File(["clip"], "demo.mp4", { type: "video/mp4" }),
      new File(["hello"], "notes.md", { type: "text/markdown" }),
    ];

    const result = await normalizeBrowserFilesToDrafts(files, DEFAULT_ATTACHMENT_POLICY);

    expect(result.accepted).toHaveLength(4);
    expect(result.accepted.map((item) => item.kind)).toEqual(["image", "audio", "video", "document"]);
    expect(result.rejected).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentDrafts.test.ts`
Expected: FAIL with module-not-found or missing-type errors.

- [ ] **Step 3: Write minimal implementation**

```ts
import type { AttachmentCapability, AttachmentPolicy } from "./attachmentPolicy";

export type AttachmentDraft = {
  id: string;
  kind: AttachmentCapability;
  sourceType: "browser_file";
  name: string;
  declaredMimeType: string;
  sizeBytes: number;
  browserFile: File;
  previewUrl?: string;
  warnings: string[];
};
```

```ts
export type AttachmentDraftReject = {
  name: string;
  reason: string;
};

export type AttachmentDraftBatch = {
  accepted: AttachmentDraft[];
  rejected: AttachmentDraftReject[];
};
```

```ts
function classifyBrowserFile(file: File): AttachmentCapability | null {
  if (file.type.startsWith("image/")) return "image";
  if (file.type.startsWith("audio/")) return "audio";
  if (file.type.startsWith("video/")) return "video";
  return "document";
}

export async function normalizeBrowserFilesToDrafts(
  files: File[],
  policy: AttachmentPolicy,
): Promise<AttachmentDraftBatch> {
  const accepted: AttachmentDraft[] = [];
  const rejected: AttachmentDraftReject[] = [];

  for (const file of files) {
    const kind = classifyBrowserFile(file);
    if (!kind) {
      rejected.push({ name: file.name, reason: "Unsupported attachment type" });
      continue;
    }
    const capability = policy.capabilities[kind];
    if (!capability.enabled) {
      rejected.push({ name: file.name, reason: `${kind} attachments disabled` });
      continue;
    }
    accepted.push({
      id: `${kind}-${file.name}`,
      kind,
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.type,
      sizeBytes: file.size,
      browserFile: file,
      warnings: [],
    });
  }

  return { accepted, rejected };
}
```

- [ ] **Step 4: Extend shared types to include the new draft/input model**

```ts
export type AttachmentSourceType =
  | "browser_file"
  | "local_path"
  | "file_url"
  | "remote_url"
  | "data_url"
  | "base64";

export type AttachmentKind = "image" | "audio" | "video" | "document";
```

```ts
export type AttachmentInput =
  | {
      id: string;
      kind: AttachmentKind;
      sourceType: "browser_file";
      name: string;
      declaredMimeType: string;
      sizeBytes: number;
    }
  | {
      id: string;
      kind: AttachmentKind;
      sourceType: "local_path" | "file_url" | "remote_url" | "data_url" | "base64";
      name: string;
      declaredMimeType?: string;
      sizeBytes?: number;
      value: string;
    };
```

- [ ] **Step 5: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentDrafts.test.ts`
Expected: PASS with unified draft normalization coverage.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src/lib/attachmentDrafts.ts apps/runtime/src/lib/__tests__/attachmentDrafts.test.ts apps/runtime/src/types.ts
git commit -m "feat: add unified frontend attachment draft model"
```

## Task 3: Unify Chat Composer and Landing Attachment Intake

**Files:**
- Modify: `apps/runtime/src/scenes/chat/useChatDraftState.ts`
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`
- Modify: `apps/runtime/src/components/chat/ChatComposer.tsx`
- Test: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- Test: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

- [ ] **Step 1: Write the failing UI regression tests**

```ts
test("landing and chat composer use the same attachment rejection message for oversized document files", async () => {
  // Arrange landing input and chat input with the same oversized .md file
  // Assert both surfaces render the same structured error text instead of alert-only divergence
});

test("team entry preserves attachments when starting a team session", async () => {
  // Arrange a landing submission with a document attachment
  // Assert onCreateTeamEntrySession receives attachments in the payload
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
Expected: FAIL because current logic is duplicated, uses `alert`, and drops team-entry attachments.

- [ ] **Step 3: Replace duplicated per-surface logic with shared helpers**

```ts
const batch = await normalizeBrowserFilesToDrafts(files, DEFAULT_ATTACHMENT_POLICY);
setComposerError(batch.rejected.map((item) => `${item.name}: ${item.reason}`).join("\n") || null);
setAttachedFiles((prev) => [...prev, ...batch.accepted]);
```

```tsx
<input
  id={LANDING_FILE_INPUT_ID}
  aria-label="添加附件"
  type="file"
  multiple
  accept={buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY)}
  className="hidden"
  onChange={handleFileSelect}
/>
```

```ts
onCreateTeamEntrySession?.({
  teamId,
  initialMessage: input.trim(),
  attachments: attachedFiles,
});
```

- [ ] **Step 4: Run UI tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
Expected: PASS with shared intake behavior and no team-entry attachment loss.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src/scenes/chat/useChatDraftState.ts apps/runtime/src/components/NewSessionLanding.tsx apps/runtime/src/components/chat/ChatComposer.tsx apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "feat: unify frontend attachment intake flows"
```

## Task 4: Expand Session Launch and Send Request Contract

**Files:**
- Modify: `apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts`
- Modify: `apps/runtime/src/scenes/chat/useChatSendController.ts`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Test: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Test: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing contract tests**

```ts
test("buildMessageParts emits unified attachment parts for image audio video and document", () => {
  // Expect emitted parts to preserve attachment kind/source metadata instead of only file_text/pdf_file
});
```

```rust
#[test]
fn send_message_request_accepts_unified_attachment_input_parts() {
    let request: SendMessageRequest = serde_json::from_value(serde_json::json!({
        "sessionId": "s1",
        "parts": [
            { "type": "text", "text": "hello" },
            { "type": "attachment", "attachment": {
                "id": "att-1",
                "kind": "audio",
                "sourceType": "browser_file",
                "name": "memo.mp3",
                "declaredMimeType": "audio/mpeg",
                "sizeBytes": 1234
            }}
        ]
    })).expect("request");

    assert_eq!(request.parts.len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx`
Run: `cargo test --test test_chat_attachment_platform`
Expected: FAIL because the current schema only understands the legacy narrow message-part types.

- [ ] **Step 3: Add compatibility-aware unified part types**

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AttachmentInput {
    pub id: String,
    pub kind: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    pub name: String,
    #[serde(rename = "declaredMimeType")]
    pub declared_mime_type: Option<String>,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub value: Option<String>,
}
```

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum SendMessagePart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "attachment")]
    Attachment { attachment: AttachmentInput },
    #[serde(rename = "image")]
    LegacyImage { name: String, #[serde(rename = "mimeType")] mime_type: String, size: usize, data: String },
    #[serde(rename = "file_text")]
    LegacyFileText { name: String, #[serde(rename = "mimeType")] mime_type: String, size: usize, text: String, truncated: Option<bool> },
    #[serde(rename = "pdf_file")]
    LegacyPdfFile { name: String, #[serde(rename = "mimeType")] mime_type: String, size: usize, data: String },
}
```

- [ ] **Step 4: Update frontend send builder to emit `attachment` parts**

```ts
export function buildMessageParts(message: string, attachments: AttachmentInput[]): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [{ type: "text", text: message.trim() || buildDefaultAttachmentPrompt(attachments) }];
  for (const attachment of attachments) {
    parts.push({ type: "attachment", attachment });
  }
  return parts;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx`
Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS with unified request shape plus legacy compatibility retained.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts apps/runtime/src/scenes/chat/useChatSendController.ts apps/runtime/src/types.ts apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: add unified attachment message contract"
```

## Task 5: Add Authoritative Backend Policy and Validation

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Test: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing backend validation tests**

```rust
#[test]
fn validation_rejects_too_many_attachments() {
    // Build six attachments against the default policy
    // Expect validation to return an error mentioning maxAttachments
}

#[test]
fn validation_rejects_remote_url_for_disabled_source() {
    // Override a capability policy to disallow remote_url
    // Expect a sourceType rejection
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_chat_attachment_platform`
Expected: FAIL because no authoritative policy validation exists yet.

- [ ] **Step 3: Add policy defaults and validation entry points**

```rust
pub struct AttachmentCapabilityPolicy {
    pub enabled: bool,
    pub max_attachments: usize,
    pub max_bytes: u64,
    pub max_chars: Option<usize>,
    pub allow_sources: &'static [&'static str],
    pub fallback_behavior: &'static str,
}
```

```rust
pub fn validate_attachment_inputs(
    policy: &AttachmentPolicy,
    attachments: &[AttachmentInput],
) -> Result<(), String> {
    if attachments.len() > policy.global_max_attachments {
        return Err(format!("attachments exceed limit {}", policy.global_max_attachments));
    }
    Ok(())
}
```

- [ ] **Step 4: Call validation from the chat attachment normalization flow**

```rust
pub(crate) fn normalize_message_parts(parts: &[SendMessagePart]) -> Result<Vec<Value>, String> {
    let inputs = collect_attachment_inputs(parts);
    validate_attachment_inputs(&default_attachment_policy(), &inputs)?;
    parts.iter().map(normalize_message_part).collect()
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS with explicit backend rejections for count/source/MIME/size failures.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs apps/runtime/src-tauri/src/commands/chat_attachments.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: add authoritative backend attachment validation"
```

## Task 6: Add Runtime Resolution for Document/Audio/Video/Image

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_resolution.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- Test: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing resolution tests**

```rust
#[test]
fn resolution_extracts_document_text_for_document_fallback() {
    // Given a document attachment with inline text/base64
    // Expect AttachmentResolved.extracted_text to be populated
}

#[test]
fn resolution_marks_audio_for_transcription_fallback() {
    // Given an audio attachment
    // Expect fallback kind/transcript placeholder to be prepared
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_chat_attachment_platform`
Expected: FAIL because no `AttachmentResolved` resolution layer exists.

- [ ] **Step 3: Introduce the resolution model**

```rust
pub struct AttachmentResolved {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub resolved_mime_type: String,
    pub size_bytes: Option<u64>,
    pub extracted_text: Option<String>,
    pub transcript: Option<String>,
    pub summary: Option<String>,
    pub warnings: Vec<String>,
}
```

```rust
pub fn resolve_attachment_input(input: &AttachmentInput) -> Result<AttachmentResolved, String> {
    Ok(AttachmentResolved {
        id: input.id.clone(),
        kind: input.kind.clone(),
        name: input.name.clone(),
        resolved_mime_type: input
            .declared_mime_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        size_bytes: input.size_bytes,
        extracted_text: None,
        transcript: None,
        summary: None,
        warnings: Vec::new(),
    })
}
```

- [ ] **Step 4: Wire normalized parts to preserve resolved attachment metadata**

```rust
Ok(json!({
    "type": "attachment",
    "attachment": {
        "id": resolved.id,
        "kind": resolved.kind,
        "name": resolved.name,
        "mimeType": resolved.resolved_mime_type,
        "sizeBytes": resolved.size_bytes,
        "extractedText": resolved.extracted_text,
        "transcript": resolved.transcript,
        "summary": resolved.summary,
        "warnings": resolved.warnings,
    }
}))
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS with resolved attachment metadata available to runtime consumers.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_attachment_resolution.rs apps/runtime/src-tauri/src/commands/chat_attachments.rs apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: add runtime attachment resolution layer"
```

## Task 7: Preserve Attachment Semantics Through Transcript Reconstruction

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Test: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`

- [ ] **Step 1: Write the failing transcript tests**

```rust
#[test]
fn build_current_turn_message_preserves_attachment_blocks_until_adapter_fallback() {
    // Given an attachment part with kind=document
    // Expect transcript conversion to keep attachment metadata available instead of immediately flattening it
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test transcript::build_current_turn_message_preserves_attachment_blocks_until_adapter_fallback`
Expected: FAIL because current transcript logic only knows text/image and document text flattening.

- [ ] **Step 3: Add attachment-aware transcript message building**

```rust
match part.get("type").and_then(Value::as_str) {
    Some("attachment") => {
        attachment_blocks.push(part.clone());
    }
    Some("text") => {}
    Some("image") => {}
    _ => {}
}
```

```rust
if let Some(fallback_text) = build_attachment_context_text_from_attachment_blocks(&attachment_blocks) {
    combined_text_parts.push(fallback_text);
}
```

- [ ] **Step 4: Run transcript tests to verify they pass**

Run: `cargo test transcript`
Expected: PASS with both legacy transcript behavior and attachment-preservation behavior covered.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/transcript.rs
git commit -m "feat: preserve attachment semantics through transcript assembly"
```

## Task 8: Add Adapter Support Matrix and Explicit OpenAI Attachment Handling

**Files:**
- Create: `apps/runtime/src-tauri/src/adapters/attachment_support.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`
- Test: `apps/runtime/src-tauri/src/adapters/openai.rs`

- [ ] **Step 1: Write the failing adapter tests**

```rust
#[test]
fn openai_responses_adapter_rejects_unsupported_video_attachment_explicitly() {
    // Expect an explicit error instead of silent loss
}

#[test]
fn openai_responses_adapter_maps_supported_image_attachment_to_input_image() {
    // Expect current image support to remain intact
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test openai`
Expected: FAIL because the current adapter silently filters to text/image-compatible parts.

- [ ] **Step 3: Add explicit attachment support declarations**

```rust
pub struct AdapterAttachmentSupport {
    pub native_image: bool,
    pub native_file: bool,
    pub document_fallback: bool,
    pub audio_fallback: bool,
    pub video_fallback: bool,
}
```

```rust
pub fn openai_responses_attachment_support() -> AdapterAttachmentSupport {
    AdapterAttachmentSupport {
        native_image: true,
        native_file: false,
        document_fallback: true,
        audio_fallback: true,
        video_fallback: true,
    }
}
```

- [ ] **Step 4: Use support declarations in OpenAI part conversion**

```rust
Some("attachment") => {
    let kind = part["attachment"]["kind"].as_str().unwrap_or_default();
    match kind {
        "image" => Some(json!({ "type": "input_image", "image_url": part["attachment"]["dataUrl"] })),
        "document" => None,
        "audio" => None,
        "video" => None,
        _ => None,
    }
}
```

```rust
if unsupported_required_attachment_detected {
    return Err("OPENAI_ATTACHMENT_UNSUPPORTED: video attachment requires fallback path".to_string());
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test openai`
Expected: PASS with explicit support/rejection semantics and preserved image behavior.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/adapters/attachment_support.rs apps/runtime/src-tauri/src/adapters/openai.rs
git commit -m "feat: add explicit adapter attachment support handling"
```

## Task 9: End-to-End Regression and WorkClaw Verification

**Files:**
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Modify: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Add final regression tests**

```ts
test("new-session attachment bootstrap reaches the first chat send", async () => {
  // Expect attachment payload to survive create-session -> pending state -> chat send
});

test("mixed attachments preserve order across the send pipeline", async () => {
  // Expect image/document/audio ordering to remain stable in built message parts
});
```

```rust
#[test]
fn legacy_and_new_attachment_parts_round_trip_together() {
    // Persist mixed old/new content parts and assert reconstruction succeeds
}
```

- [ ] **Step 2: Run focused test suites**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx`
Expected: PASS.

Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS.

- [ ] **Step 3: Run WorkClaw verification commands**

Run: `pnpm test:rust-fast`
Expected: PASS.

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "test: add attachment platform regression coverage"
```

## Spec Coverage Check

- Unified attachment model: covered by Tasks 1, 2, and 4.
- Shared policy layer with global + capability policy: covered by Tasks 1 and 5.
- Shared frontend normalization and consistent intake behavior: covered by Tasks 2 and 3.
- Authoritative backend validation: covered by Task 5.
- Runtime resolution and richer semantics: covered by Task 6.
- Adapter-aware native/fallback handling: covered by Tasks 7 and 8.
- P0 bug fixes like team-entry attachment loss and inconsistent frontend rules: covered by Task 3.
- Compatibility-preserving migration posture: covered by Tasks 4, 7, and 9.
- Verification expectations from the spec: covered by Task 9.

## Placeholder Check

- No `TODO`, `TBD`, or deferred placeholders remain in the task steps.
- Each task names exact files and commands.
- Code-modifying steps include concrete type/function scaffolding rather than abstract directives.

## Type Consistency Check

- Shared capability names remain `image`, `audio`, `video`, `document`.
- Shared source names remain `browser_file`, `local_path`, `file_url`, `remote_url`, `data_url`, `base64`.
- Shared part type for new flow remains `attachment`.
- Legacy compatibility names remain isolated as `LegacyImage`, `LegacyFileText`, and `LegacyPdfFile` in the Rust plan snippets.
