# Chat Thinking Block Design

**Context**

The chat experience currently exposes only a lightweight global execution state such as `正在分析任务`, but it does not provide the user-facing "thinking" presentation pattern that modern AI products have trained users to expect. The target experience is closer to products that show a compact `思考中` state, allow the user to expand the model's real reasoning content while it streams, and preserve that reasoning for later review after the reply completes.

The key constraint is that the expandable content must be genuine model reasoning output when available. It cannot be substituted with task summaries, tool logs, or fabricated explanatory text. If reasoning is unavailable for a provider or model, the UI may still show a compact `思考中` state, but it must not pretend there is expandable thought content.

**Goal**

Introduce a first-class `ThinkingBlock` experience for assistant replies:

- show `思考中` as soon as the assistant enters a thinking phase
- stream real reasoning content into an expandable block when available
- keep the thinking block visually secondary to the final answer
- preserve the thinking content in conversation history for later expansion
- show a completed state such as `已思考 2.4s` after reasoning ends

**Product Decisions**

- Use `思考中` as the active-state label because it matches established user mental models in mainstream AI products.
- Use `已思考 x.xs` as the completed-state label.
- Make the thinking block visible to all users by default.
- Default the thinking block to collapsed.
- Allow the user to expand and inspect real reasoning content during streaming and after completion.
- Preserve reasoning in history so it can be expanded again later.
- Keep the final answer as the primary content region. Reasoning is supportive context, not the main output.

**Non-Goals**

- Do not fabricate reasoning from task states, tool activity, or system prompts.
- Do not expose internal orchestration logs as "thinking".
- Do not make reasoning part of the final answer body.
- Do not add reasoning export, copy, search, or permission controls in the first iteration.
- Do not over-design the visuals with heavy animation or oversized loading placeholders.

**Recommended UX**

The assistant reply should become a composite message with two distinct layers:

1. `ThinkingBlock`
2. `AnswerBlock`

The `ThinkingBlock` always renders above the final answer when reasoning exists or when the model is currently in a thinking phase.

### Empty Thinking State

When the assistant has entered thinking but no reasoning content has arrived yet:

- render a compact header row only
- show `思考中`
- show a subtle lightweight activity indicator
- do not show `0.0s`
- do not show a large empty expanded panel
- do not show the expand affordance until actual reasoning content exists

### Streaming Thinking State

When reasoning chunks begin arriving:

- keep the header label as `思考中`
- show elapsed time in the header
- enable expand and collapse interaction
- keep the block collapsed by default
- if the user expands it, append incoming reasoning chunks live into the body

### Answer Streaming State

When answer tokens begin arriving:

- render the final answer below the thinking block
- continue appending reasoning into the expanded body if reasoning is still active
- keep the answer visually dominant

Answer and reasoning are independent streams and may overlap in time.

### Completed State

When reasoning finishes:

- change the header label to `已思考 x.xs`
- stop the activity animation
- keep the expand affordance if reasoning content exists
- preserve the full reasoning content for later expansion

Do not render a second redundant "completed thinking" card. The header state change is sufficient.

### Interrupted State

If the request fails or reasoning stops unexpectedly:

- keep any already received reasoning text
- change the state label to `思考中断`
- still allow expansion if content exists
- let the main error presentation remain separate

### History Replay

When a conversation is reopened:

- show `已思考 x.xs` for assistant replies that have persisted reasoning
- keep the thinking block collapsed by default
- allow the user to expand and review the historical reasoning again

**Visual Hierarchy**

The thinking block should look clearly secondary to the final answer:

- container: soft neutral background, light border, rounded corners
- header text: subdued neutral text, compact size
- reasoning body: smaller or lighter than final answer body, comfortable line height
- max body height with internal scroll for long reasoning
- final answer maintains stronger contrast and larger visual weight

Avoid these anti-patterns:

- making the thinking block as visually heavy as the answer
- rendering multiple duplicate state labels such as `思考中`, `已思考`, and `完成思考`
- showing a large empty panel before any reasoning arrives
- using aggressive shimmer or overly decorative motion

**Information Architecture**

Reasoning must be modeled as a separate content channel from the final answer. The UI may visually group both under one assistant reply, but the underlying data should distinguish:

- assistant status
- assistant reasoning content
- assistant answer content

This separation is required for:

- clean rendering
- stable history replay
- correct copy and export boundaries later
- provider compatibility across different reasoning formats

**Protocol Shape**

Recommended runtime events:

- `assistant-reasoning-started`
- `assistant-reasoning-delta`
- `assistant-reasoning-completed`
- `assistant-reasoning-interrupted`
- existing answer delta events continue separately

The existing global `agent-state-event` may remain as a high-level execution indicator, but it should not be the source of truth for expandable reasoning content.

**Persistence Shape**

Each assistant reply should be able to persist:

- reasoning status
- reasoning duration
- reasoning full text
- final answer text

The first iteration does not need chunk-level persistence if full-text persistence is sufficient for replay.

**Success Criteria**

- The user sees `思考中` shortly after an assistant turn begins.
- Real reasoning content can stream into an expandable area without contaminating the answer body.
- The final answer remains the visually dominant content.
- Completed replies show `已思考 x.xs` when reasoning existed.
- Historical assistant replies can re-open the preserved reasoning.
- Models without reasoning do not show an empty fake thinking panel.
- Interrupted runs preserve received reasoning and mark it clearly.
