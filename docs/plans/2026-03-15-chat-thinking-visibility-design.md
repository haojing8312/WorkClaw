# Chat Thinking Visibility Design

**Problem**

In long chat sessions, the transient `思考中` indicator is rendered above the full message history when a new assistant turn has entered the thinking phase but has not emitted answer tokens yet. The chat viewport, however, is normally pinned near the bottom after the user sends a new message. This makes the system look unresponsive even though the assistant has already started working.

**Scope**

This fix should stay narrow:

- Only change where the transient streaming thinking state is rendered
- Keep the existing `ThinkingBlock` component and persisted historical reasoning UI
- Keep the current auto-scroll behavior to the bottom
- Do not add duplicated status banners, toast notifications, or forced scroll-to-top behavior

**Chosen Design**

Render the transient thinking-only state in the same bottom streaming assistant bubble that later hosts streamed answer tokens, instead of rendering a standalone thinking block before the historical message list.

**Behavior**

- After the user sends a message, the bottom of the chat remains the interaction focal point.
- If the assistant is in a thinking phase and no answer tokens have arrived yet, show `思考中` in the bottom streaming bubble.
- If reasoning text arrives before answer tokens, the same bottom bubble may expose the expandable reasoning affordance there.
- Once answer tokens begin, continue rendering the thinking block above the streamed answer content inside that same bottom bubble.
- Persisted assistant messages in history continue to render their own historical `ThinkingBlock` exactly as they do today.

**Why This Design**

- It keeps feedback at the point where the user is already looking after sending a message.
- It avoids rendering the same transient state in two places.
- It preserves the current reasoning model and rendering structure with minimal code churn.
- It avoids disruptive viewport jumps that would happen with scroll-to-top style fixes.

**Non-Goals**

- Do not redesign the overall message layout.
- Do not change stored reasoning persistence or replay semantics.
- Do not change the `ThinkingBlock` copy, timing labels, or expand/collapse behavior.
- Do not introduce sticky headers or floating global loading overlays for this fix.

**Validation**

- In a long session with existing history, entering the thinking state should render `思考中` at the bottom streaming region, not above the message list.
- When answer tokens arrive, the thinking block should remain attached to the active streaming bubble.
- Historical assistant replies with persisted reasoning should continue to render normally.
