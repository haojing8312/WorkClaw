# Rust DB Split Design

**Goal:** Turn [db.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/db.rs) into a maintainable Rust runtime boundary by separating connection/bootstrap logic, current schema creation, legacy migrations, and startup seed work. The split must preserve current SQLite behavior and make future table/column changes land in predictable places instead of continuing to accrete in one root file.

## Why `db.rs` Is The Next Rust Target

[db.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/db.rs) is still above the Rust `800` split-design threshold and currently mixes four different responsibilities:

- connection-pool creation and PRAGMA setup
- current schema creation through many `CREATE TABLE` and `CREATE INDEX` statements
- legacy compatibility through a long chain of `ALTER TABLE` and schema-fallback helpers
- startup seed behavior such as builtin skill sync, default app settings, and builtin team template initialization

That mix creates two risks:

- AI-native work keeps appending new schema changes wherever there is room, rather than following a stable migration pattern
- a future split could accidentally move table creation, legacy compatibility, and default data into inconsistent layers

This design treats `db.rs` as infrastructure bootstrap, not as the long-term home for every schema evolution step.

## Current Boundary Map

### What The File Owns Today

- build builtin skill manifest JSON
- sync builtin skills into `installed_skills`
- open the SQLite database in the app data directory
- apply PRAGMA settings
- create the current runtime tables and indexes
- run historical `ALTER TABLE` compatibility steps
- insert default app settings
- seed builtin team templates
- run the one-off `ensure_im_thread_sessions_channel_column` legacy helper
- keep a small test block for builtin-skill sync and a legacy-schema migration case

### What Makes It Hard To Extend Safely

- new tables and new columns currently land in the same file even though they have different lifecycles
- some startup defaults are true schema concerns, while others are seed concerns, but they are interleaved
- legacy compatibility checks use a different style than the table-creation path, yet both are mixed into one long function
- adding one new column encourages another `ALTER TABLE` near unrelated domain tables, which is exactly the kind of sprawl we are trying to stop

## Recommended Design

### 1. Keep `db.rs` As The Bootstrap Shell

The root [db.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/db.rs) should end up owning only:

- connection-pool creation
- app data directory and DB URL resolution
- PRAGMA setup
- top-level bootstrap order
- a very small number of infrastructure-only helpers that truly belong at the root

Its main job should be to call a small sequence such as:

1. `apply_schema(&pool)`
2. `apply_migrations(&pool)`
3. `seed_runtime_defaults(&pool, &app_dir)`

That keeps the order obvious and makes future schema work land in one of three explicit lanes.

### 2. Split By Schema Lifecycle, Not By Domain Yet

The first safe split is not per domain. It is per lifecycle:

- `schema.rs`: the current expected database shape
- `migrations.rs`: compatibility steps from older shapes to the current one
- `seed.rs`: repeatable startup data sync and default settings

This is the lowest-risk split because it matches how the code already behaves today. It also avoids prematurely fragmenting the DB layer into many domain files before we have stable governance rules for migrations.

### 3. Add Migration Governance Rules

The most important outcome is not just a smaller file. It is a stable rule for where future DB work belongs.

#### New Table Rule

When adding a brand-new table:

- add the canonical `CREATE TABLE IF NOT EXISTS` statement to `schema.rs`
- add indexes for that table to `schema.rs`
- only add migration work to `migrations.rs` if existing databases need special backfill or shape repair beyond the idempotent create step

#### New Column Rule

When adding a column to an existing table:

- add the `ALTER TABLE ... ADD COLUMN ...` compatibility step to `migrations.rs`
- if the running code needs to tolerate old databases before the migration lands, add a legacy-schema fallback check in `migrations.rs` or in the query path
- do not bury new column additions inside `schema.rs` alone, because existing databases will not see them

#### New Seed Rule

When adding repeatable default data:

- place it in `seed.rs`
- keep it idempotent by using `INSERT OR IGNORE`, `ON CONFLICT`, or equivalent upsert semantics
- do not mix it into `schema.rs` or `migrations.rs`

#### Startup-Critical Compatibility Rule

For startup-critical reads such as session lists, IM bindings, employee routing, or similar SQLite-backed startup paths:

- every new column or shape dependency must ship with a compatibility step
- add or preserve at least one legacy-schema test proving older DBs still load

This rule already exists at the repo level; `db.rs` should become the place where that rule is structurally enforced.

## Suggested Module Layout

The first split should create a new sibling directory under `apps/runtime/src-tauri/src/`:

- `db/schema.rs`
  - `CREATE TABLE IF NOT EXISTS ...`
  - `CREATE INDEX IF NOT EXISTS ...`
  - grouped helper functions such as `create_session_tables`, `create_im_tables`, `create_employee_tables`, `create_cache_tables`
- `db/migrations.rs`
  - historical `ALTER TABLE` statements
  - `pragma_table_info` checks
  - one-off schema repair helpers like `ensure_im_thread_sessions_channel_column`
- `db/seed.rs`
  - builtin skill manifest sync
  - default app settings inserts
  - builtin team template seed startup call

The root [db.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/db.rs) then becomes the bootstrap shell that imports those modules and coordinates the order.

## Recommended Bootstrap Order

The startup order should be explicit and stable:

1. open the pool
2. apply PRAGMA settings
3. create the current schema
4. run migrations for older databases
5. insert or sync startup defaults
6. return the pool

That order keeps new installs simple while still allowing older installs to converge safely.

## Testing Strategy

### Keep Existing Root Tests, Then Move Them

The current low-risk test set should survive the split:

- builtin skill sync upserts manifest metadata
- builtin skill sync is idempotent
- legacy `im_thread_sessions.channel` migration works

After the split stabilizes, these tests can move to:

- `db/seed.rs` tests for builtin skill sync
- `db/migrations.rs` tests for legacy schema compatibility

### Add Legacy-Schema Tests When New Columns Matter

Whenever a new migration changes a startup-critical table:

- create a minimal in-memory legacy schema without the new column
- run the migration function
- assert the new column or fallback path now exists

This is the concrete guardrail that keeps AI-assisted schema evolution from breaking old user databases.

## Smallest Safe Split Order

1. Extract `seed.rs` first.
   - Lowest risk because builtin skill sync and default settings are already mostly self-contained.
2. Extract `migrations.rs` second.
   - Move `ALTER TABLE` steps and legacy helpers without changing the startup call order.
3. Extract `schema.rs` third.
   - Move `CREATE TABLE` and `CREATE INDEX` blocks into grouped helpers.
4. Thin the root `init_db` function into a simple bootstrap sequence.
5. Move tests to the nearest module only after the split compiles cleanly and current behavior is locked.

This order reduces risk because it peels off the most cohesive behavior first and avoids rewriting the root bootstrap flow too early.

## Non-Goals

- Do not redesign the actual SQLite schema in this split.
- Do not rename tables or columns.
- Do not switch to a new migration framework.
- Do not jump straight to per-domain DB files in the first pass.

Those may be future improvements, but they are not necessary to make `db.rs` governable right now.

## Success Criteria

This split is successful when:

- [db.rs](/d:/code/WorkClaw/apps/runtime/src-tauri/src/db.rs) falls below the `800` split-design threshold
- new tables, new columns, and new seed data each have a clearly documented landing zone
- the startup bootstrap order is easier to read than today
- existing SQLite behavior stays intact
- at least one legacy-schema test still proves backward compatibility for startup-critical data
