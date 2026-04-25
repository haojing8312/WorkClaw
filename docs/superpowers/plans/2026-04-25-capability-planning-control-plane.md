# Capability Planning Control Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the brittle workspace-image keyword spike with a structured resource and capability planning path that lets WorkClaw analyze local images through a dedicated vision tool.

**Architecture:** Keep explicit image attachments on the existing native vision route. Add a capability registry, workspace resource context, and `vision_analyze` runtime tool so ambiguous natural-language requests over workspace images are handled through tool-calling instead of hardcoded phrase routing.

**Tech Stack:** Rust workspace crates, Tauri runtime, serde_json, existing WorkClaw tool registry/effective tool set, existing attachment policy constants, real-agent eval harness.

---

## Implementation Status 2026-04-25

- Implemented: removed the workspace-image keyword spike, added the capability registry, added `TurnResourceContext`, registered `vision_analyze`, wired workspace image resources into runtime notes plus tool recommendations, and added a real-agent eval scenario for workspace image analysis.
- Verified: focused `runtime-chat-app` tests pass, Tauri runtime compiles, `vision_analyze` and resource-context lib tests compile with `--no-run`, and `pnpm test:rust-fast` passes.
- Real-agent eval note: `agent-evals/scenarios/workspace_image_set_vision_2026_04_25.yaml` requires an OpenAI-compatible default eval provider so `vision_analyze` can resolve the seeded `vision` route.

---

## File Structure

- Modify `packages/runtime-chat-app/src/preparation.rs`: remove the spike's Chinese workspace-image keyword expansion; keep structured image-part inference.
- Modify `packages/runtime-chat-app/tests/capability.rs`: replace phrase-list tests with structured behavior tests.
- Create `packages/runtime-chat-app/src/capabilities.rs`: define stable capability registry records.
- Modify `packages/runtime-chat-app/src/lib.rs`: export capability registry helpers.
- Create `apps/runtime/src-tauri/src/agent/runtime/resource_context.rs`: resolve compact workspace resource summaries.
- Modify `apps/runtime/src-tauri/src/agent/runtime/mod.rs`: expose `resource_context`.
- Create `apps/runtime/src-tauri/src/agent/tools/vision_analyze.rs`: implement the runtime image-analysis tool.
- Modify `apps/runtime/src-tauri/src/agent/tools/mod.rs`: export `VisionAnalyzeTool`.
- Modify `apps/runtime/src-tauri/src/agent/runtime/kernel/tool_registry_setup.rs`: register `vision_analyze`.
- Modify `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`: append resource runtime notes and recommend `vision_analyze` when image resources exist.
- Modify `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`: compute `TurnResourceContext` once per turn and pass it to tool setup.
- Modify `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`: store resource/planning summaries in `TurnContext`.
- Add `agent-evals/scenarios/workspace_image_set_vision.yaml`: regression scenario for the 2026-04-25 missed-vision transcript.

Do not edit packaging, release metadata, installer files, or vendor sync lanes.

---

### Task 1: Remove The Keyword Spike

**Files:**
- Modify: `packages/runtime-chat-app/src/preparation.rs`
- Modify: `packages/runtime-chat-app/tests/capability.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat.rs`
- Modify: `apps/runtime/src-tauri/src/commands/chat_policy.rs`

- [ ] **Step 1: Replace the phrase-list test with structured image-part coverage**

In `packages/runtime-chat-app/tests/capability.rs`, remove `infers_vision_from_chinese_image_reading_requests` and add:

```rust
#[test]
fn infers_vision_from_structured_image_parts_not_workspace_phrases() {
    let parts = vec![serde_json::json!({
        "type": "image",
        "data": "data:image/png;base64,abc"
    })];

    assert_eq!(
        infer_capability_from_message_parts(&parts, "普通聊天"),
        "vision"
    );
    assert_eq!(infer_capability_from_user_message("读取这些图片"), "chat");
}
```

- [ ] **Step 2: Run the focused package test and verify it fails while spike code remains**

Run:

```powershell
cargo test --manifest-path packages/runtime-chat-app/Cargo.toml --test capability infers_vision_from_structured_image_parts_not_workspace_phrases -- --exact
```

Expected: fails because the spike currently returns `vision` for `读取这些图片`.

- [ ] **Step 3: Remove workspace-image phrase expansion from capability inference**

In `packages/runtime-chat-app/src/preparation.rs`, keep explicit modality terms already supported before the spike, but remove helper logic that pairs image nouns with verbs such as `读取`, `分析`, `内容`, `看看`, and remove direct phrases such as `读取这些图片`, `每个图片`, `所有图片`, and `这些图片`.

The resulting function should behave like:

```rust
pub fn infer_capability_from_user_message(message: &str) -> &'static str {
    let lower = message.to_lowercase();
    if lower.contains("识图")
        || lower.contains("看图")
        || lower.contains("图片理解")
        || lower.contains("vision")
        || lower.contains("analyze image")
    {
        return "vision";
    }
    if lower.contains("生图")
        || lower.contains("生成图片")
        || lower.contains("image generation")
    {
        return "image_generation";
    }
    if lower.contains("语音转文字") || lower.contains("speech to text") {
        return "speech_to_text";
    }
    if lower.contains("文字转语音") || lower.contains("text to speech") {
        return "text_to_speech";
    }
    "chat"
}
```

- [ ] **Step 4: Remove `auto_attach_workspace_images_for_vision_request` from `chat.rs`**

Delete the spike helper and tests added around `auto_attach_workspace_images_for_vision_request`. Also remove imports that only served the helper: base64, `Path`, attachment policy constants, `infer_capability_from_user_message`, and `serde_json::json` if no longer needed in that file.

- [ ] **Step 5: Restore `apps/runtime/src-tauri/src/commands/chat_policy.rs` test-only inference**

Remove the same workspace-image phrase expansion from the duplicate test helper in `chat_policy.rs` so route policy tests stay aligned with `runtime-chat-app`.

- [ ] **Step 6: Verify the cleanup**

Run:

```powershell
cargo test --manifest-path packages/runtime-chat-app/Cargo.toml --test capability infers_vision_from_structured_image_parts_not_workspace_phrases -- --exact
```

Expected: pass.

---

### Task 2: Add Capability Registry Records

**Files:**
- Create: `packages/runtime-chat-app/src/capabilities.rs`
- Modify: `packages/runtime-chat-app/src/lib.rs`
- Test: `packages/runtime-chat-app/src/capabilities.rs`

- [ ] **Step 1: Create the capability registry module**

Create `packages/runtime-chat-app/src/capabilities.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityInputKind {
    Text,
    Image,
    Audio,
    Video,
    Document,
    WorkspaceResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityRouteKind {
    MainModel,
    NativeVision,
    RuntimeTool,
    AuxiliaryModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub input_kinds: &'static [CapabilityInputKind],
    pub preferred_routes: &'static [CapabilityRouteKind],
    pub recommended_tools: &'static [&'static str],
}

pub const CAPABILITIES: &[CapabilityDefinition] = &[
    CapabilityDefinition {
        id: "chat",
        label: "Chat",
        input_kinds: &[CapabilityInputKind::Text],
        preferred_routes: &[CapabilityRouteKind::MainModel],
        recommended_tools: &[],
    },
    CapabilityDefinition {
        id: "vision",
        label: "Vision",
        input_kinds: &[CapabilityInputKind::Image, CapabilityInputKind::WorkspaceResource],
        preferred_routes: &[CapabilityRouteKind::NativeVision, CapabilityRouteKind::RuntimeTool],
        recommended_tools: &["vision_analyze"],
    },
    CapabilityDefinition {
        id: "image_generation",
        label: "Image generation",
        input_kinds: &[CapabilityInputKind::Text],
        preferred_routes: &[CapabilityRouteKind::RuntimeTool],
        recommended_tools: &[],
    },
    CapabilityDefinition {
        id: "speech_to_text",
        label: "Speech to text",
        input_kinds: &[CapabilityInputKind::Audio],
        preferred_routes: &[CapabilityRouteKind::AuxiliaryModel],
        recommended_tools: &[],
    },
    CapabilityDefinition {
        id: "text_to_speech",
        label: "Text to speech",
        input_kinds: &[CapabilityInputKind::Text],
        preferred_routes: &[CapabilityRouteKind::RuntimeTool],
        recommended_tools: &[],
    },
];

pub fn capability_definition(id: &str) -> Option<&'static CapabilityDefinition> {
    CAPABILITIES.iter().find(|capability| capability.id == id)
}

pub fn recommended_tools_for_capability(id: &str) -> &'static [&'static str] {
    capability_definition(id)
        .map(|capability| capability.recommended_tools)
        .unwrap_or(&[])
}
```

- [ ] **Step 2: Add module tests**

Append:

```rust
#[cfg(test)]
mod tests {
    use super::{
        capability_definition, recommended_tools_for_capability, CapabilityInputKind,
        CapabilityRouteKind,
    };

    #[test]
    fn vision_capability_declares_workspace_resource_and_tool_route() {
        let vision = capability_definition("vision").expect("vision capability");

        assert!(vision.input_kinds.contains(&CapabilityInputKind::WorkspaceResource));
        assert!(vision.preferred_routes.contains(&CapabilityRouteKind::RuntimeTool));
        assert_eq!(recommended_tools_for_capability("vision"), &["vision_analyze"]);
    }

    #[test]
    fn unknown_capability_has_no_recommended_tools() {
        assert_eq!(recommended_tools_for_capability("missing"), &[] as &[&str]);
    }
}
```

- [ ] **Step 3: Export the module**

In `packages/runtime-chat-app/src/lib.rs`, add:

```rust
pub mod capabilities;
```

- [ ] **Step 4: Run the focused tests**

Run:

```powershell
cargo test --manifest-path packages/runtime-chat-app/Cargo.toml capabilities
```

Expected: pass.

---

### Task 3: Resolve Workspace Resource Context

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/runtime/resource_context.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/mod.rs`

- [ ] **Step 1: Add resource context types and resolver**

Create `apps/runtime/src-tauri/src/agent/runtime/resource_context.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkspaceImageResourceSummary {
    pub id: String,
    pub source: String,
    pub count: usize,
    pub sample_names: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TurnResourceContext {
    pub work_dir: Option<String>,
    pub workspace_images: Option<WorkspaceImageResourceSummary>,
}

pub(crate) fn resolve_turn_resource_context(work_dir: Option<&str>) -> TurnResourceContext {
    let Some(work_dir) = work_dir.map(str::trim).filter(|value| !value.is_empty()) else {
        return TurnResourceContext::default();
    };
    let root = Path::new(work_dir);
    let workspace_images = summarize_top_level_images(root);
    TurnResourceContext {
        work_dir: Some(work_dir.to_string()),
        workspace_images,
    }
}

fn summarize_top_level_images(root: &Path) -> Option<WorkspaceImageResourceSummary> {
    let mut files = Vec::<(String, u64)>::new();
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_supported_image_path(&path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        files.push((name, size));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        return None;
    }
    let total_bytes = files.iter().map(|(_, size)| *size).sum();
    let sample_names = files.iter().take(5).map(|(name, _)| name.clone()).collect();
    Some(WorkspaceImageResourceSummary {
        id: "workspace.images".to_string(),
        source: "workspace_top_level".to_string(),
        count: files.len(),
        sample_names,
        total_bytes,
    })
}

fn is_supported_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        })
        .unwrap_or(false)
}
```

- [ ] **Step 2: Add tests for deterministic resource summaries**

Append tests in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::resolve_turn_resource_context;
    use std::fs;

    #[test]
    fn resolve_turn_resource_context_summarizes_top_level_images() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("b.jpg"), b"bb").expect("write jpg");
        fs::write(temp.path().join("a.png"), b"a").expect("write png");
        fs::write(temp.path().join("notes.md"), b"text").expect("write text");

        let context = resolve_turn_resource_context(temp.path().to_str());
        let images = context.workspace_images.expect("workspace images");

        assert_eq!(images.id, "workspace.images");
        assert_eq!(images.count, 2);
        assert_eq!(images.sample_names, vec!["a.png", "b.jpg"]);
        assert_eq!(images.total_bytes, 3);
    }
}
```

- [ ] **Step 3: Export the module**

In `apps/runtime/src-tauri/src/agent/runtime/mod.rs`, add:

```rust
pub(crate) mod resource_context;
```

- [ ] **Step 4: Run the focused test**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml resource_context --lib
```

Expected: pass.

---

### Task 4: Add `vision_analyze` Runtime Tool Skeleton

**Files:**
- Create: `apps/runtime/src-tauri/src/agent/tools/vision_analyze.rs`
- Modify: `apps/runtime/src-tauri/src/agent/tools/mod.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/tool_registry_setup.rs`

- [ ] **Step 1: Add the tool module**

Create `apps/runtime/src-tauri/src/agent/tools/vision_analyze.rs`:

```rust
use crate::agent::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct VisionAnalyzeTool;

impl VisionAnalyzeTool {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Tool for VisionAnalyzeTool {
    fn name(&self) -> &str {
        "vision_analyze"
    }

    fn description(&self) -> &str {
        "Analyze local workspace images with the configured vision route. Use this when the user asks to inspect, describe, compare, or summarize image files in the current workspace. Do not use browser tools or shell scripts just to read local image contents."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["workspace_image_set", "local_paths"]
                        },
                        "selection": {
                            "type": "string",
                            "enum": ["all"]
                        },
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["type"]
                },
                "prompt": {
                    "type": "string",
                    "description": "What to analyze or describe for the selected images."
                },
                "batch_size": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 8,
                    "default": 4
                }
            },
            "required": ["target", "prompt"]
        })
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let prompt = input
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("prompt is required"))?;
        let target = input.get("target").ok_or_else(|| anyhow!("target is required"))?;
        let target_type = target
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("target.type is required"))?;

        let paths = match target_type {
            "workspace_image_set" => collect_workspace_images(ctx)?,
            "local_paths" => collect_local_paths(target)?,
            other => return Err(anyhow!("unsupported target.type: {other}")),
        };
        if paths.is_empty() {
            return Err(anyhow!("VISION_NO_IMAGES: no image files matched the target"));
        }

        Ok(format!(
            "VISION_ANALYZE_PENDING: {} image(s) selected for prompt: {}",
            paths.len(),
            prompt
        ))
    }
}

fn collect_workspace_images(ctx: &ToolContext) -> Result<Vec<PathBuf>> {
    let root = ctx
        .work_dir
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("WORKDIR_REQUIRED: current workspace is not available"))?;
    let entries = fs::read_dir(&root)?;
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_image_path(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn collect_local_paths(target: &Value) -> Result<Vec<PathBuf>> {
    let values = target
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("target.paths is required for local_paths"))?;
    Ok(values
        .iter()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_file() && is_supported_image_path(path))
        .collect())
}

fn is_supported_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif"
            )
        })
        .unwrap_or(false)
}
```

This skeleton intentionally returns `VISION_ANALYZE_PENDING`. A later task wires it to the actual vision route. This gives tests and tool selection a stable contract before provider integration.

- [ ] **Step 2: Export and register the tool**

In `apps/runtime/src-tauri/src/agent/tools/mod.rs`, add:

```rust
pub mod vision_analyze;
pub use vision_analyze::VisionAnalyzeTool;
```

In `apps/runtime/src-tauri/src/agent/runtime/kernel/tool_registry_setup.rs`, add `VisionAnalyzeTool` to the imports and register it near other runtime tools:

```rust
params
    .agent_executor
    .registry()
    .register(Arc::new(VisionAnalyzeTool::new()));
```

- [ ] **Step 3: Add a tool unit test**

In `vision_analyze.rs`, add a test that creates a temp workdir with two images and asserts the pending result mentions `2 image(s)`.

- [ ] **Step 4: Run the focused test**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml vision_analyze --lib
```

Expected: pass.

---

### Task 5: Inject Resource Notes And Recommend `vision_analyze`

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs`
- Modify: `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`

- [ ] **Step 1: Add `resource_context` to tool setup params**

Extend `ToolSetupParams` in `tool_setup.rs`:

```rust
pub resource_context: Option<&'a crate::agent::runtime::resource_context::TurnResourceContext>,
```

Update all call sites to pass `Some(&resource_context)` when available and `None` in tests that do not construct one.

- [ ] **Step 2: Build a factual runtime note**

Add this helper to `tool_setup.rs`:

```rust
fn resource_context_runtime_notes(
    context: Option<&crate::agent::runtime::resource_context::TurnResourceContext>,
) -> Vec<String> {
    let Some(context) = context else {
        return Vec::new();
    };
    let Some(images) = context.workspace_images.as_ref() else {
        return Vec::new();
    };
    vec![format!(
        "当前工作区包含 {} 个顶层图片文件，示例：{}。当用户要求查看、读取、分析、描述或比较这些图片时，使用 `vision_analyze`，target.type 设为 `workspace_image_set`，selection 设为 `all`。不要为了读取本地图片内容改用 browser 工具或 shell/Python。",
        images.count,
        images.sample_names.join(", ")
    )]
}
```

- [ ] **Step 3: Merge resource notes into capability snapshot notes**

Before `CapabilitySnapshot::build_with_tool_plan`, merge:

```rust
let mut supplemental_notes = params.supplemental_runtime_notes.to_vec();
supplemental_notes.extend(resource_context_runtime_notes(params.resource_context));
```

Pass `&supplemental_notes` to `merge_runtime_notes`.

- [ ] **Step 4: Recommend `vision_analyze` through existing discovery candidates**

When `resource_context.workspace_images.is_some()`, append a `ToolDiscoveryCandidateRecord` for `vision_analyze` with primary stage if the tool exists in the manifest. Reuse the existing `EffectiveToolSet::apply_recommended_tools` path rather than adding a separate allowlist.

- [ ] **Step 5: Add tests**

Add a `tool_setup` unit test that builds a fake effective tool set with `vision_analyze`, passes a resource context with workspace images, and asserts:

- `vision_analyze` is in `allowed_tools`
- the system prompt includes `当前工作区包含`
- the note says not to use browser/shell/Python for local image contents

- [ ] **Step 6: Run focused tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml tool_setup --lib
```

Expected: pass.

---

### Task 6: Wire The Real Vision Execution Path

**Files:**
- Modify: `apps/runtime/src-tauri/src/agent/tools/vision_analyze.rs`
- Reuse: `apps/runtime/src-tauri/src/commands/chat_attachments.rs`
- Reuse: `packages/runtime-chat-app/src/routing.rs`

- [ ] **Step 1: Replace pending output with route-backed execution**

Update `VisionAnalyzeTool` so it can call the configured `vision` route policy. The implementation should:

- load route candidates for `vision`
- validate image count and size with existing image attachment policy constants
- encode each selected image as a provider image part for the configured vision route
- call the existing provider adapter path used for native vision when possible
- return a per-file markdown result

- [ ] **Step 2: Preserve explicit failure modes**

Return clear errors:

```text
VISION_MODEL_NOT_CONFIGURED: no enabled vision route is configured
VISION_NO_IMAGES: no image files matched the target
VISION_IMAGE_TOO_LARGE: <file> exceeds the configured image size limit
VISION_PARTIAL_SKIPPED: analyzed N image(s), skipped M image(s)
```

- [ ] **Step 3: Add policy tests**

Add tests for:

- no configured vision route
- empty workspace
- file over size limit
- multiple images batched under limit

- [ ] **Step 4: Run focused tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml vision_analyze --lib
```

Expected: pass.

---

### Task 7: Add Regression Eval

**Files:**
- Add: `agent-evals/scenarios/workspace_image_set_vision.yaml`

- [ ] **Step 1: Add a local-only scenario definition**

Create a scenario with anonymous capability IDs and no secrets:

```yaml
id: workspace_image_set_vision
prompt:
  - role: user
    content: 当前工作空间里有什么
  - role: user
    content: 你读取这些图片，并告诉我每个图片的内容
expect:
  selected_tool: vision_analyze
  forbidden_tools:
    - browser_launch
    - browser_navigate
    - exec
  min_image_count: 1
```

- [ ] **Step 2: Run the real-agent eval**

Run:

```powershell
pnpm eval:agent-real --scenario workspace_image_set_vision
```

Expected: pass with `selected_tool=vision_analyze` and no browser/shell fallback.

---

### Task 8: Final Verification

**Files:**
- All touched files from Tasks 1-7

- [ ] **Step 1: Format focused Rust code**

Run:

```powershell
cargo fmt --manifest-path packages/runtime-chat-app/Cargo.toml
rustfmt --edition 2021 apps/runtime/src-tauri/src/agent/runtime/resource_context.rs apps/runtime/src-tauri/src/agent/tools/vision_analyze.rs apps/runtime/src-tauri/src/agent/runtime/tool_setup.rs apps/runtime/src-tauri/src/agent/runtime/kernel/turn_preparation.rs apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs
```

Expected: formatting completes. If full Tauri `cargo fmt` still hits the known helper-path issue, record it and keep focused `rustfmt` output.

- [ ] **Step 2: Run package tests**

Run:

```powershell
cargo test --manifest-path packages/runtime-chat-app/Cargo.toml --test capability
```

Expected: pass.

- [ ] **Step 3: Run Tauri focused tests**

Run:

```powershell
cargo test --manifest-path apps/runtime/src-tauri/Cargo.toml resource_context vision_analyze tool_setup --lib
```

Expected: pass.

- [ ] **Step 4: Run sidecar/runtime verification**

Run:

```powershell
pnpm test:rust-fast
pnpm eval:agent-real --scenario workspace_image_set_vision
```

Expected: pass, or document any environment-only failures with the exact error text.

---

## Self-Review Notes

- The plan deliberately avoids a general LLM side planner in P0. Tool-calling plus structured resource context is the smallest elegant path that fixes the reported class of failures.
- The old native image attachment path remains intact.
- Browser automation is explicitly not a fallback for local image analysis.
- The capability registry starts static but creates a clean migration path to plugin/manifest-provided capabilities later.
- The current spike files are called out in Task 1 so implementation starts by removing brittle behavior, not layering on top of it.
