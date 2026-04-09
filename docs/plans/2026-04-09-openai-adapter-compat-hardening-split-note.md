# OpenAI Adapter Compat Hardening Split Note

## Why this note exists
- [apps/runtime/src-tauri/src/adapters/openai.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/adapters/openai.rs) is already above the 800-line feature-work threshold.
- The Qwen/OpenAI-compatible tool-calling fix needs one more round of transport-aware compat hardening, but that work still belongs next to the current OpenAI streaming/parser code paths.

## Immediate split direction
- Keep the current request/stream entrypoints in `openai.rs`.
- Add only small helper functions in this slice for:
  - endpoint-family-aware request flags
  - tool schema normalization
  - tool-call parsing repair
- Avoid adding more provider-specific branching directly into the main request loop than needed for this compatibility fix.

## Not in this slice
- No large mechanical split of `openai.rs`.
- No new provider plugin system.
- No migration of Qwen/OpenRouter traffic to a new Responses-only transport.

## Next split candidates
1. Extract request-body builders into `src/adapters/openai_request.rs` once completions vs responses payload shaping grows again.
2. Extract stream/tool-call repair into `src/adapters/openai_stream.rs` once parser and repair tests stabilize.
3. Extract schema normalization into `src/adapters/openai_schema.rs` once multiple providers need different JSON Schema policies.
