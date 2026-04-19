# WorkClaw Attachment Platform Remaining Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the real attachment-platform foundation that is still missing in the current repo so WorkClaw can move from narrow image/text/pdf handling to a unified runtime attachment pipeline.

**Architecture:** Keep the existing attachment UX working while introducing a shared frontend policy plus draft layer, a compatibility-safe `attachment` send contract, and an authoritative Tauri validation and resolution path. Defer provider flattening until transcript and adapter fallback decisions, while preserving legacy stored message compatibility.

**Tech Stack:** React, TypeScript, Tauri, Rust, serde, Vitest, Rust integration tests, existing WorkClaw runtime adapters.

---

## Current Truth Snapshot

- `apps/runtime/src/lib/chatAttachments.ts` still hardcodes narrow limits and narrow type buckets.
- `apps/runtime/src/scenes/chat/useChatDraftState.ts` and `apps/runtime/src/components/NewSessionLanding.tsx` still duplicate attachment intake logic and still use `alert(...)`.
- `apps/runtime/src/types.ts` still reflects legacy `PendingAttachment` shapes (`image`, `text-file`, `pdf-file`) rather than a unified platform model.
- `apps/runtime/src-tauri/src/commands/chat.rs` and `apps/runtime/src-tauri/src/commands/chat_attachments.rs` still assume the old message-part flow.
- `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs` does not exist yet.

## File Map

### Existing files to modify

- `apps/runtime/src/types.ts`
- `apps/runtime/src/lib/chatAttachments.ts`
- `apps/runtime/src/scenes/chat/useChatDraftState.ts`
- `apps/runtime/src/components/NewSessionLanding.tsx`
- `apps/runtime/src/scenes/chat/useChatSendController.ts`
- `apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts`
- `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- `apps/runtime/src-tauri/src/commands/chat.rs`
- `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- `apps/runtime/src-tauri/src/commands/mod.rs`
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

## Task 1: Add Shared Frontend Attachment Policy

**Files:**
- Create: `apps/runtime/src/lib/attachmentPolicy.ts`
- Create: `apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts`
- Modify: `apps/runtime/src/lib/chatAttachments.ts`

- [ ] **Step 1: Write the failing policy tests**

```ts
import {
  DEFAULT_ATTACHMENT_POLICY,
  buildFileInputAccept,
  resolveAttachmentCapability,
} from "../attachmentPolicy";

describe("attachmentPolicy", () => {
  test("defines capability defaults for image audio video and document", () => {
    expect(resolveAttachmentCapability(DEFAULT_ATTACHMENT_POLICY, "image").maxBytes).toBe(10 * 1024 * 1024);
    expect(resolveAttachmentCapability(DEFAULT_ATTACHMENT_POLICY, "audio").fallbackBehavior).toBe("transcribe");
    expect(resolveAttachmentCapability(DEFAULT_ATTACHMENT_POLICY, "video").enabled).toBe(true);
    expect(resolveAttachmentCapability(DEFAULT_ATTACHMENT_POLICY, "document").allowedExtensions).toContain(".pdf");
  });

  test("builds one accept string from policy", () => {
    const accept = buildFileInputAccept(DEFAULT_ATTACHMENT_POLICY);
    expect(accept).toContain(".md");
    expect(accept).toContain("image/png");
    expect(accept).toContain("audio/mpeg");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentPolicy.test.ts`
Expected: FAIL with module-not-found or missing export errors.

- [ ] **Step 3: Implement the shared policy module**

```ts
export type AttachmentKind = "image" | "audio" | "video" | "document";
export type AttachmentSourceType =
  | "browser_file"
  | "local_path"
  | "file_url"
  | "remote_url"
  | "data_url"
  | "base64";

export type AttachmentCapabilityPolicy = {
  enabled: boolean;
  maxAttachments: number;
  maxBytes: number;
  maxChars?: number;
  allowedMimeTypes: string[];
  allowedExtensions: string[];
  allowSources: AttachmentSourceType[];
  fallbackBehavior: "native" | "extract_text" | "transcribe" | "summarize" | "reject";
};
```

```ts
export const DEFAULT_ATTACHMENT_POLICY = {
  global: { maxAttachments: 5 },
  capabilities: {
    image: {
      enabled: true,
      maxAttachments: 3,
      maxBytes: 10 * 1024 * 1024,
      allowedMimeTypes: ["image/png", "image/jpeg", "image/webp", "image/gif"],
      allowedExtensions: [".png", ".jpg", ".jpeg", ".webp", ".gif"],
      allowSources: ["browser_file"],
      fallbackBehavior: "native",
    },
    audio: {
      enabled: true,
      maxAttachments: 2,
      maxBytes: 25 * 1024 * 1024,
      allowedMimeTypes: ["audio/mpeg", "audio/wav", "audio/mp4", "audio/webm"],
      allowedExtensions: [".mp3", ".wav", ".m4a", ".webm"],
      allowSources: ["browser_file"],
      fallbackBehavior: "transcribe",
    },
    video: {
      enabled: true,
      maxAttachments: 1,
      maxBytes: 50 * 1024 * 1024,
      allowedMimeTypes: ["video/mp4", "video/webm", "video/quicktime"],
      allowedExtensions: [".mp4", ".webm", ".mov"],
      allowSources: ["browser_file"],
      fallbackBehavior: "summarize",
    },
    document: {
      enabled: true,
      maxAttachments: 5,
      maxBytes: 20 * 1024 * 1024,
      maxChars: 200_000,
      allowedMimeTypes: ["text/plain", "text/markdown", "application/pdf", "application/json"],
      allowedExtensions: [".txt", ".md", ".pdf", ".json", ".csv", ".xls", ".xlsx", ".doc", ".docx"],
      allowSources: ["browser_file"],
      fallbackBehavior: "extract_text",
    },
  },
} as const;
```

```ts
export function resolveAttachmentCapability(policy: typeof DEFAULT_ATTACHMENT_POLICY, kind: AttachmentKind) {
  return policy.capabilities[kind];
}

export function buildFileInputAccept(policy: typeof DEFAULT_ATTACHMENT_POLICY): string {
  return Object.values(policy.capabilities)
    .flatMap((entry) => [...entry.allowedExtensions, ...entry.allowedMimeTypes])
    .filter((value, index, all) => all.indexOf(value) === index)
    .join(",");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentPolicy.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src/lib/attachmentPolicy.ts apps/runtime/src/lib/__tests__/attachmentPolicy.test.ts apps/runtime/src/lib/chatAttachments.ts
git commit -m "feat: add shared attachment policy"
```

## Task 2: Add Unified Frontend Draft Normalization

**Files:**
- Create: `apps/runtime/src/lib/attachmentDrafts.ts`
- Create: `apps/runtime/src/lib/__tests__/attachmentDrafts.test.ts`
- Modify: `apps/runtime/src/types.ts`

- [ ] **Step 1: Write the failing draft normalization tests**

```ts
import { normalizeBrowserFilesToDrafts } from "../attachmentDrafts";
import { DEFAULT_ATTACHMENT_POLICY } from "../attachmentPolicy";

describe("attachmentDrafts", () => {
  test("normalizes image audio video and document files into unified drafts", async () => {
    const files = [
      new File(["img"], "cover.png", { type: "image/png" }),
      new File(["voice"], "memo.mp3", { type: "audio/mpeg" }),
      new File(["clip"], "demo.mp4", { type: "video/mp4" }),
      new File(["sheet"], "budget.xlsx", { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" }),
    ];

    const result = await normalizeBrowserFilesToDrafts(files, DEFAULT_ATTACHMENT_POLICY);

    expect(result.accepted.map((item) => item.kind)).toEqual(["image", "audio", "video", "document"]);
    expect(result.rejected).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentDrafts.test.ts`
Expected: FAIL with missing module or missing type errors.

- [ ] **Step 3: Add the unified draft and input types**

```ts
export type AttachmentDraft = {
  id: string;
  kind: AttachmentKind;
  sourceType: "browser_file";
  name: string;
  declaredMimeType: string;
  sizeBytes: number;
  browserFile: File;
  previewUrl?: string;
  warnings: string[];
};

export type AttachmentInput = {
  id: string;
  kind: AttachmentKind;
  sourceType: AttachmentSourceType;
  name: string;
  declaredMimeType?: string;
  sizeBytes?: number;
  sourcePayload?: string;
};
```

```ts
export async function normalizeBrowserFilesToDrafts(files: File[], policy: typeof DEFAULT_ATTACHMENT_POLICY) {
  const accepted: AttachmentDraft[] = [];
  const rejected: Array<{ name: string; reason: string }> = [];

  for (const file of files) {
    const kind = classifyAttachmentKind(file);
    const capability = resolveAttachmentCapability(policy, kind);
    if (file.size > capability.maxBytes) {
      rejected.push({ name: file.name, reason: `${file.name} exceeds ${capability.maxBytes} bytes` });
      continue;
    }
    accepted.push({
      id: createAttachmentId("draft"),
      kind,
      sourceType: "browser_file",
      name: file.name,
      declaredMimeType: file.type,
      sizeBytes: file.size,
      browserFile: file,
      previewUrl: kind === "image" ? await readFileAsDataUrl(file) : undefined,
      warnings: [],
    });
  }

  return { accepted, rejected };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentDrafts.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src/lib/attachmentDrafts.ts apps/runtime/src/lib/__tests__/attachmentDrafts.test.ts apps/runtime/src/types.ts
git commit -m "feat: add unified attachment drafts"
```

## Task 3: Route Landing And Chat Through The Same Intake Flow

**Files:**
- Modify: `apps/runtime/src/scenes/chat/useChatDraftState.ts`
- Modify: `apps/runtime/src/components/NewSessionLanding.tsx`
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`

- [ ] **Step 1: Write the failing surface regression tests**

```ts
test("landing and composer reject the same oversized document with the same structured message", async () => {
  // Assert both flows render the same message instead of divergent alerts.
});

test("team entry preserves attachments in the launch payload", async () => {
  // Assert onCreateTeamEntrySession receives attachments.
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
Expected: FAIL because the flows still duplicate logic, still use `alert`, and still drop team-entry attachments.

- [ ] **Step 3: Replace duplicated intake code with shared normalization**

```ts
const batch = await normalizeBrowserFilesToDrafts(files, DEFAULT_ATTACHMENT_POLICY);
setComposerError(batch.rejected.map((item) => item.reason).join("\n") || null);
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/runtime/src/scenes/chat/useChatDraftState.ts apps/runtime/src/components/NewSessionLanding.tsx apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx
git commit -m "feat: unify attachment intake across landing and chat"
```

## Task 4: Introduce A Compatibility-Safe `attachment` Send Contract

**Files:**
- Modify: `apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts`
- Modify: `apps/runtime/src/scenes/chat/useChatSendController.ts`
- Modify: `apps/runtime/src/types.ts`
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_types.rs`
- Create: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing contract tests**

```ts
test("session launch and first send preserve attachment kind and source metadata", () => {
  // Expect message parts to contain `type: "attachment"` entries.
});
```

```rust
#[test]
fn send_message_request_deserializes_unified_attachment_parts() {
    let request: SendMessageRequest = serde_json::from_value(serde_json::json!({
        "sessionId": "s1",
        "parts": [
            { "type": "text", "text": "hello" },
            { "type": "attachment", "attachment": {
                "id": "att-1",
                "kind": "document",
                "sourceType": "browser_file",
                "name": "budget.xlsx",
                "declaredMimeType": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
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
Expected: FAIL because the current contract only understands the legacy narrow attachment shapes.

- [ ] **Step 3: Add shared message-part types with legacy compatibility**

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
    #[serde(rename = "sourcePayload")]
    pub source_payload: Option<String>,
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

- [ ] **Step 4: Update the frontend builder to emit the new contract**

```ts
export function buildMessageParts(message: string, attachments: AttachmentInput[]) {
  const parts = [{ type: "text" as const, text: message.trim() }];
  for (const attachment of attachments) {
    parts.push({ type: "attachment" as const, attachment });
  }
  return parts;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pnpm --dir apps/runtime exec vitest run src/__tests__/App.session-create-flow.test.tsx`
Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src/scenes/useGeneralSessionLaunchCoordinator.ts apps/runtime/src/scenes/chat/useChatSendController.ts apps/runtime/src/types.ts apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src-tauri/src/commands/chat.rs apps/runtime/src-tauri/src/commands/chat_attachment_types.rs apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: add unified attachment send contract"
```

## Task 5: Add Authoritative Backend Policy, Validation, And Resolution

**Files:**
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs`
- Create: `apps/runtime/src-tauri/src/commands/chat_attachment_resolution.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- Modify: `apps/runtime/src-tauri/src/commands/mod.rs`
- Modify: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing backend tests**

```rust
#[test]
fn validation_rejects_oversized_document() {
    // Expect a size-based rejection from backend validation.
}

#[test]
fn validation_rejects_disabled_source_type() {
    // Expect a sourceType rejection when policy disallows the incoming source.
}

#[test]
fn resolution_preserves_attachment_kind_for_runtime_consumers() {
    // Expect AttachmentResolved.kind to remain `audio` or `video` rather than collapsing early.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test test_chat_attachment_platform`
Expected: FAIL because no centralized Rust policy, validation, or resolution modules exist yet.

- [ ] **Step 3: Implement policy and validation**

```rust
pub struct AttachmentCapabilityPolicy {
    pub enabled: bool,
    pub max_attachments: usize,
    pub max_bytes: u64,
    pub allow_sources: &'static [&'static str],
    pub fallback_behavior: &'static str,
}
```

```rust
pub fn validate_attachment_inputs(policy: &AttachmentPolicy, attachments: &[AttachmentInput]) -> Result<(), String> {
    if attachments.len() > policy.global_max_attachments {
        return Err(format!("attachments exceed limit {}", policy.global_max_attachments));
    }
    for attachment in attachments {
        let capability = policy.capability(&attachment.kind)?;
        if !capability.allow_sources.contains(&attachment.source_type.as_str()) {
          return Err(format!("source type {} not allowed for {}", attachment.source_type, attachment.kind));
        }
        if let Some(size_bytes) = attachment.size_bytes {
          if size_bytes > capability.max_bytes {
            return Err(format!("attachment {} exceeds {}", attachment.name, capability.max_bytes));
          }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Implement resolution and wire it into normalization**

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
pub(crate) fn normalize_message_parts(parts: &[SendMessagePart]) -> Result<Vec<Value>, String> {
    let attachments = collect_attachment_inputs(parts);
    validate_attachment_inputs(&default_attachment_policy(), &attachments)?;
    parts.iter().map(normalize_message_part).collect()
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/commands/chat_attachment_policy.rs apps/runtime/src-tauri/src/commands/chat_attachment_validation.rs apps/runtime/src-tauri/src/commands/chat_attachment_resolution.rs apps/runtime/src-tauri/src/commands/chat_attachments.rs apps/runtime/src-tauri/src/commands/mod.rs apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: add backend attachment validation and resolution"
```

## Task 6: Preserve Attachment Semantics Through Transcript And Adapter Boundaries

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/transcript.rs`
- Create: `apps/runtime/src-tauri/src/adapters/attachment_support.rs`
- Modify: `apps/runtime/src-tauri/src/adapters/openai.rs`
- Modify: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Write the failing transcript and adapter tests**

```rust
#[test]
fn transcript_keeps_attachment_blocks_until_fallback_is_needed() {
    // Expect document/audio/video attachment metadata to remain available before adapter conversion.
}

#[test]
fn openai_adapter_rejects_unsupported_attachment_explicitly() {
    // Expect a clear error instead of silent loss.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test transcript`
Run: `cargo test openai`
Expected: FAIL because the current runtime still collapses non-image attachments too early.

Windows note:

- If direct `cargo test --lib <filter>` execution hits the historical local `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` startup issue, use the isolated compile gate instead:
- `node scripts/run-cargo-isolated.mjs attachment-transcript -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
- Treat this as the reliable Windows compile verification for transcript/adapter changes before broader Rust validation.

- [ ] **Step 3: Preserve attachment blocks in transcript assembly**

```rust
match part.get("type").and_then(Value::as_str) {
    Some("attachment") => attachment_blocks.push(part.clone()),
    Some("text") => combined_text_parts.push(part["text"].as_str().unwrap_or_default().to_string()),
    _ => {}
}
```

- [ ] **Step 4: Add explicit adapter support declarations**

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
if unsupported_required_attachment_detected {
    return Err("OPENAI_ATTACHMENT_UNSUPPORTED".to_string());
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test transcript`
Run: `cargo test openai`
Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/runtime/src-tauri/src/agent/runtime/transcript.rs apps/runtime/src-tauri/src/adapters/attachment_support.rs apps/runtime/src-tauri/src/adapters/openai.rs apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "feat: preserve attachment semantics through adapters"
```

## Task 7: Final Regression Verification

**Files:**
- Modify: `apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx`
- Modify: `apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx`
- Modify: `apps/runtime/src/__tests__/App.session-create-flow.test.tsx`
- Modify: `apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs`

- [ ] **Step 1: Run focused frontend verification**

Run: `pnpm --dir apps/runtime exec vitest run src/lib/__tests__/attachmentPolicy.test.ts src/lib/__tests__/attachmentDrafts.test.ts src/components/__tests__/NewSessionLanding.test.tsx src/components/__tests__/ChatView.side-panel-redesign.test.tsx src/__tests__/App.session-create-flow.test.tsx`
Expected: PASS.

- [ ] **Step 2: Run focused Rust verification**

Run: `cargo test --test test_chat_attachment_platform`
Expected: PASS.

Windows fallback:

- If the local machine still shows the historical `0xc0000139` or shared-target contamination during direct Rust test execution, run:
- `node scripts/run-cargo-isolated.mjs attachment-platform -- test --manifest-path apps/runtime/src-tauri/Cargo.toml --lib --no-run`
- plus `pnpm test:rust-fast`
- Treat that combination as the honest Windows-side Rust verification signal until direct libtest execution is healthy again.

- [ ] **Step 3: Run WorkClaw verification baseline**

Run: `pnpm test:rust-fast`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/runtime/src/components/__tests__/NewSessionLanding.test.tsx apps/runtime/src/components/__tests__/ChatView.side-panel-redesign.test.tsx apps/runtime/src/__tests__/App.session-create-flow.test.tsx apps/runtime/src-tauri/tests/test_chat_attachment_platform.rs
git commit -m "test: add attachment platform regression coverage"
```

## Spec Coverage Check

- Unified policy layer: covered by Task 1.
- Unified draft and input model: covered by Task 2.
- Shared landing/chat intake and team-entry preservation: covered by Task 3.
- New compatibility-safe send contract: covered by Task 4.
- Backend policy authority and runtime resolution: covered by Task 5.
- Transcript and adapter explicitness: covered by Task 6.
- Verification for frontend and Rust surfaces: covered by Task 7.

## Type Consistency Check

- Attachment capability names remain `image`, `audio`, `video`, `document`.
- Attachment source names remain `browser_file`, `local_path`, `file_url`, `remote_url`, `data_url`, `base64`.
- The new message-part type remains `attachment`.
- Legacy message-part compatibility remains isolated inside Rust deserialization rather than frontend draft types.

## Execution Recommendation

- Start with Task 1 and Task 2 in the same batch only if the worker can keep the shared type surface small.
- Treat Task 4 as the migration pivot. Do not start Task 5 or Task 6 until the new `attachment` contract is compiling end-to-end.
- If a quick win is needed before the full platform, Task 3 can land after Task 2 to remove the current divergent UX even before backend authority is finished.
