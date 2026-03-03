# Local Patches

- `src/routing/resolve-route.ts`: import `agent-scope-lite` instead of full `agent-scope` to avoid pulling full OpenClaw runtime dependency tree into sidecar.
- `src/routing/bindings.ts`: replaced channel/account binding helpers with sidecar-local minimal version (same matching semantics used by routing priority tests).
- Added sidecar-local shim files required by vendored routing subset:
  - `src/agents/agent-scope-lite.ts`
  - `src/config/config.ts`
  - `src/globals.ts`
  - `src/logger.ts`
  - `src/infra/prototype-keys.ts`
  - `src/sessions/session-key-utils.ts`
