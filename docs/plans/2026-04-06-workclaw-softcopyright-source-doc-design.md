# WorkClaw Soft Copyright Source Document Design

**Date:** 2026-04-06
**Status:** approved in-session

## Goal

Generate a submission-ready Microsoft Word source-program document for the full WorkClaw product, aligned with common PRC software copyright filing practice:

- A4 paper
- source-program body in a monospaced font
- at least 50 lines per page
- first 30 pages plus last 30 pages from a continuous full-source listing

## Scope

Include WorkClaw first-party product source from the main runtime delivery surface:

- `scripts/*.mjs`
- `apps/runtime/src/**/*`
- `apps/runtime/src-tauri/src/**/*`
- `apps/runtime/src-tauri/build.rs`
- `apps/runtime/sidecar/src/**/*`
- `packages/*/src/**/*`

Exclude:

- `node_modules`
- vendored third-party mirrors
- local config, secrets, logs, temp files, and generated outputs
- tests, fixtures, examples, screenshots, binary assets, and release artifacts

## Document Strategy

1. Build a deterministic full source listing from tracked files in a fixed directory order.
2. Split the listing into logical pages using a fixed line budget so the first and last submission pages come from a continuous source sequence.
3. Emit only the first 30 and last 30 pages into the `.docx`.
4. Use A4 page size, narrow-but-readable margins, `Courier New` body text, exact line spacing, page header, and numeric footer.
5. Insert file path separators between files for reviewer readability without changing underlying code content.

## Verification

Verify the generated document with local evidence:

- confirm the `.docx` exists
- inspect OOXML page settings for A4 layout
- confirm the generator emitted exactly 60 source pages
- confirm the configured source-line budget is at least 50 lines per page
- if Microsoft Word automation is available, open the document and confirm pagination from Word itself

## Deliverable

- `temp/softcopyright/WorkClaw-源程序文档.docx`
- local generator and verification helpers under `temp/softcopyright/`
