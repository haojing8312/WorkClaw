# OpenClaw Core Vendor (WorkClaw)

This folder vendors the OpenClaw routing subset used by WorkClaw sidecar.

## Upstream Sync

1. Prepare an upstream checkout and set `OPENCLAW_UPSTREAM_PATH`.
2. Run:
   - `node scripts/sync-openclaw-core.mjs`
3. Check and update:
   - `apps/runtime/sidecar/vendor/openclaw-core/UPSTREAM_COMMIT`
   - `apps/runtime/sidecar/vendor/openclaw-core/PATCHES.md`
4. Run regression tests:
   - `pnpm --dir apps/runtime/sidecar test`
   - `cargo test --test test_openclaw_gateway --test test_openclaw_route_regression -- --nocapture`

## Notes

- WorkClaw keeps a minimal local patch layer for sidecar-only routing usage.
- See `PATCHES.md` for exact local diffs against upstream.
