# WorkClaw Capability Planning Control Plane Design

Date: 2026-04-25

## Strategy Summary

- Change surface: desktop chat runtime planning, capability selection, resource resolution, tool exposure, and media/vision execution.
- Affected modules: `packages/runtime-chat-app`, `apps/runtime/src-tauri/src/agent/runtime`, `apps/runtime/src-tauri/src/agent/tools`, attachment policy modules, session journal observability, and real-agent eval scenarios.
- Main risk: replacing brittle keyword routing without regressing existing explicit image attachments, skill routing, and ordinary chat/tool behavior.
- Recommended smallest safe path: keep explicit image attachments on the current native vision path, add a structured workspace resource context plus a `vision_analyze` tool for local image files, then fold the resulting signals into the existing `ExecutionPlan` and tool recommendation flow.
- Required verification: focused Rust unit tests, runtime-chat package tests, sidecar/runtime compile checks, and a real-agent eval that reproduces `当前工作空间里有什么-2026-04-25-1508.md`.
- Release impact: runtime behavior changes only; no packaging, installer, or vendor lane release changes expected.

## Problem

The recent spike tried to solve missed vision routing by adding more natural-language phrases to `infer_capability_from_user_message` and by auto-attaching workspace image files inside `chat.rs`.

That is the wrong long-term shape.

The failing session was not just a missing phrase. The user first asked what was in the workspace, the assistant listed image files, then the user said "读取这些图片，并告诉我每个图片的内容". At that point the runtime had no structured image parts in the user message and no first-class resource reference for "these images". The agent fell back to browser/Python attempts because the tool/model surface did not make the correct path obvious.

The underlying gap is that WorkClaw currently mixes several concerns:

- natural-language modality inference
- model capability routing
- resource reference resolution
- attachment hydration
- tool exposure
- execution-lane selection

When these are handled as scattered string checks, every new phrasing or resource source requires another code patch.

## Reference Findings

OpenClaw, Hermes, and close-code converge on the same pattern:

- ordinary user intent is mostly model-mediated through structured tool schemas and dynamic prompts
- deterministic local routing is reserved for explicit syntax or high-confidence runtime facts
- images/media are first-class resources, not text phrases
- model capabilities live in registries/catalogs, not provider-name branches
- unavailable tools/capabilities are hidden before the model sees them
- large media uses claim-check/resource references instead of eager base64 injection

Important reference anchors:

- OpenClaw dynamically assembles tools and plugin tools before policy filtering: `references/openclaw/src/agents/openclaw-tools.ts`.
- OpenClaw parses attachments into inline or `media://inbound/...` claim-check resources: `references/openclaw/src/gateway/chat-attachments.ts`.
- OpenClaw injects native images only when the active model supports them: `references/openclaw/src/agents/pi-embedded-runner/run/images.ts`.
- Hermes exposes `vision_analyze` as a normal tool routed through its auxiliary model client: `references/hermes-agent/tools/vision_tools.py`.
- Hermes filters tool definitions by registry/toolset availability: `references/hermes-agent/model_tools.py`.
- close-code stores images as structured message blocks early and converts them at provider boundaries: `F:/code/yzpd/close-code/src/utils/processUserInput/processUserInput.ts`.

## Goals

- Remove natural-language phrase expansion as the mechanism for workspace image understanding.
- Make referenced files, workspace media, and attachments visible as structured runtime resources.
- Let model tool-calling select media analysis when the request is ambiguous but the resource context is clear.
- Keep explicit image attachments on the current native vision route.
- Introduce a capability registry that describes capability requirements and preferred execution routes.
- Make routing/planning decisions observable in the session journal and real-agent eval traces.
- Keep the first implementation small enough to verify safely.

## Non-Goals

- Do not build a full universal LLM planner in the first implementation.
- Do not auto-attach every workspace image as model input.
- Do not replace the existing skill routing control plane.
- Do not change installer, packaging, release metadata, or vendor sync lanes.
- Do not make browser automation a fallback for local image analysis.

## Design Principles

- Resources first, intent second. The runtime should know what files/media are available before asking the model to act.
- Prefer schemas over keyword branches. If a capability needs natural-language guidance, put it in tool/capability descriptions and evals, not scattered Rust string lists.
- Keep deterministic routes based on structured facts: explicit image parts, explicit file paths, explicit commands, selected UI attachments, and policy state.
- Use tool-calling for ambiguous natural-language requests over workspace resources.
- Keep native vision and tool vision separate. Native vision is for explicit message images; `vision_analyze` is for local files, workspace selections, and fallback analysis.
- Make partial execution explicit. If only some images fit policy, the result must say which images were skipped and why.

## Target Architecture

### 1. Capability Registry

Add a small capability registry in `packages/runtime-chat-app`.

The registry describes stable capability IDs and their requirements:

- `chat`
- `vision`
- `image_generation`
- `speech_to_text`
- `text_to_speech`
- `workspace_file_read`
- `media_understanding`

Each definition should include:

- `id`
- `label`
- `input_kinds`
- `preferred_routes`
- `required_model_features`
- `recommended_tools`
- `fallback_behavior`

The first pass can be static Rust data. The important change is that capabilities become data records that other layers can inspect, instead of implicit string literals spread across routing code.

### 2. Resource Context

Add a runtime resource resolver in Tauri, close to turn preparation.

It should produce a compact `TurnResourceContext`:

- current workdir
- image files in the top-level workdir
- recent file-listing resources when available
- explicit attachment parts already present in the user message
- resource policy warnings such as count/size limits

The resolver must not decide user intent. It only says what resources are available and safe to offer.

Example:

```json
{
  "work_dir": "C:\\Users\\...\\Desktop\\宁夏人工智能全民素养提升培训图片",
  "resources": [
    {
      "id": "workspace.images",
      "kind": "image_set",
      "source": "workspace_top_level",
      "count": 13,
      "sample_names": ["001.jpg", "002.jpg", "003.png"],
      "total_bytes": 12400000
    }
  ]
}
```

### 3. Workspace Resource Runtime Note

Inject a short runtime note when relevant resources exist.

The note should be factual and bounded:

- current workspace contains N image files
- use `vision_analyze` when the user asks to inspect, compare, summarize, or describe those images
- do not use browser tools or shell/Python just to read local image contents
- if too many files are requested, analyze in batches or ask the user to narrow scope

This moves guidance into the prompt/tool contract where it belongs, while keeping code free of phrase lists.

### 4. `vision_analyze` Tool

Add a runtime tool for image analysis.

The tool accepts structured targets:

```json
{
  "target": {
    "type": "workspace_image_set",
    "selection": "all"
  },
  "prompt": "Describe each image.",
  "batch_size": 4
}
```

It should also support explicit local paths:

```json
{
  "target": {
    "type": "local_paths",
    "paths": ["C:\\path\\a.png", "C:\\path\\b.jpg"]
  },
  "prompt": "Describe each image."
}
```

The tool is responsible for:

- resolving paths under the current workdir unless absolute paths are explicitly permitted by existing policy
- validating MIME/extension/size using the shared attachment policy
- batching images under provider limits
- calling the configured vision route or auxiliary vision model
- returning per-file results and skipped-file diagnostics

This mirrors Hermes' `vision_analyze` shape and keeps workspace-file vision out of `chat.rs`.

### 5. Native Vision Route Stays Narrow

The current native vision route should stay for explicit message image blocks:

- pasted image
- selected image attachment
- frontend-produced image part

This path should not be used for vague workspace references because it would require the runtime to guess which files the user meant before the model has planned the action.

### 6. Execution Plan Integration

WorkClaw already has `ExecutionPlan` and `TurnContext` in `apps/runtime/src-tauri/src/agent/runtime/kernel/execution_plan.rs`.

Extend that context with optional resource/planning signals:

- `resource_context`
- `recommended_tool_query`
- `planner_notes`

The first implementation does not need a separate LLM side planner. It can:

1. resolve resources
2. recommend `vision_analyze` when image resources are present
3. expose the tool and runtime note
4. let the main model call the tool

Later, if WorkClaw needs stronger control, add a side-query `TurnPlanner` that outputs a structured `ExecutionPlan` from the same registry and resource context.

### 7. Observability

Record the following events or journal fields:

- resource context summary
- recommended tools from resource context
- whether `vision_analyze` was exposed
- whether native vision was used
- whether browser/Python was attempted for image analysis
- per-file vision analysis success/skipped counts

This makes future accuracy issues diagnosable without reading model transcripts manually.

## Expected Behavior For The Failing Session

For `D:\code\WorkClaw\temp\当前工作空间里有什么-2026-04-25-1508.md`:

1. The runtime sees a current workdir with image files.
2. It injects a compact resource note and exposes/recommends `vision_analyze`.
3. The model receives a tool schema that can analyze `workspace_image_set`.
4. The model calls `vision_analyze` instead of browser or Python.
5. The tool returns per-image descriptions.
6. If the configured model lacks vision, the tool returns a clear `VISION_MODEL_NOT_CONFIGURED` style error.

## Compatibility

- Existing explicit image attachment behavior must remain unchanged.
- Existing text/PDF large attachment handling must remain unchanged.
- Existing skill routing decisions must remain unchanged unless a skill explicitly restricts tools.
- Existing permission filters must still be able to deny `vision_analyze`.
- Sessions without workspace images should not get extra vision guidance.

## Open Questions

- Whether `vision_analyze` should use the same configured `vision` route policy as native image parts, or a separate auxiliary model preference.
- Whether top-level workdir scanning is enough for P0, or whether recursive image discovery should be opt-in.
- Whether real-agent eval should assert the exact tool name or only assert absence of browser/Python plus successful vision output.

For P0, choose the conservative answers:

- use the existing `vision` route policy
- scan only top-level workdir
- assert exact `vision_analyze` tool usage in the regression eval

