# OpenClaw IM Core Vendor (WorkClaw)

This folder reserves the future vendor lane for OpenClaw IM adapter code that is broader than the current routing subset.

## Current State

- No second channel is enabled yet.
- The lane exists so maintainers can add a new OpenClaw-backed connector without mixing upstream code directly into WorkClaw business layers.
- Until a channel is selected, `UPSTREAM_COMMIT` may remain `uninitialized`.

## Upstream Sync

1. Prepare an upstream checkout and set `OPENCLAW_IM_UPSTREAM_PATH`, or reuse `OPENCLAW_UPSTREAM_PATH`.
2. Run:
   - `node scripts/sync-openclaw-im-core.mjs`
3. Check and update:
   - `apps/runtime/sidecar/vendor/openclaw-im-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-im-core/PATCHES.md`
4. Run regression tests:
   - `node --test scripts/check-openclaw-vendor-lane.test.mjs`
   - `pnpm --dir apps/runtime/sidecar test`
   - `cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

## Notes

- Keep upstream code inside the sidecar boundary only.
- Record every local deviation in `PATCHES.md` before merging.
- Expand the sync manifest only when a concrete second connector is being integrated.
