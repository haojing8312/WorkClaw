## [LRN-20260320-001] best_practice

**Logged**: 2026-03-20T11:20:00+08:00
**Priority**: high
**Status**: promoted
**Area**: backend

### Summary
Startup-critical SQLite reads must stay compatible with legacy schemas, especially the session list path.

### Details
WorkClaw introduced `SELECT ts.channel` in the session list and session search SQL for `im_thread_sessions`, but the runtime database bootstrap and migration path did not add the `channel` column for existing databases. On upgraded installs, `list_sessions` failed with `no such column: ts.channel`, and the UI appeared to show only one session because it fell back to the last selected session snapshot in local storage. The data was still present in SQLite; the failure was schema compatibility, not session loss.

### Suggested Action
For any SQLite schema evolution that affects runtime reads, do both:
1. add an explicit migration for old databases
2. add a legacy-schema regression test for the affected read path

### Metadata
- Source: simplify-and-harden
- Related Files: AGENTS.md, apps/runtime/src-tauri/src/db.rs, apps/runtime/src-tauri/src/commands/chat_session_io.rs
- Tags: sqlite, migration, backward-compatibility, sessions, startup
- Pattern-Key: harden.sqlite_legacy_schema_reads
- Recurrence-Count: 1
- First-Seen: 2026-03-20
- Last-Seen: 2026-03-20

### Resolution
- **Resolved**: 2026-03-20T11:20:00+08:00
- **Commit/PR**: pending
- **Notes**: Promoted the durable workflow rule to `AGENTS.md` and fixed the runtime migration plus legacy-schema query fallback.

---
