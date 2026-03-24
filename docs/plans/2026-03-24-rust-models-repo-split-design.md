# Rust Models Repo Split Design

**Goal:** Reduce `apps/runtime/src-tauri/src/commands/models_repo.rs` by separating configuration persistence from read-heavy lookup/reporting logic while preserving the existing `models.rs` service construction surface.

## Why This Split

`models_repo.rs` is not a simple CRUD repository. It currently mixes:

- provider and model configuration persistence
- routing settings and capability policy writes
- catalog cache reads and provider connection lookup
- route attempt history and stats reads
- provider catalog and health-probe adapters

That makes the file harder to reason about and encourages future additions to land in the wrong lane.

## Recommended Split

Create two child modules:

- `config_repo.rs`
- `read_repo.rs`

Move the following into `config_repo.rs`:

- `ModelsConfigRepository for PoolModelsRepository`
- `ModelsConfigRepository for NullModelsRepository`
- routing settings persistence
- provider config persistence
- model config persistence
- capability routing policy persistence

Move the following into `read_repo.rs`:

- `ModelsReadRepository for PoolModelsRepository`
- `ModelsReadRepository for NullModelsRepository`
- provider key and enabled provider queries
- model catalog cache reads and writes
- route attempt logs and stats queries
- provider connection lookup

Keep these in the root file for now:

- `PoolModelsRepository`
- `RegistryProviderCatalog`
- `BuiltinProviderCatalog`
- `NullModelsRepository`
- `NullProviderCatalog`
- `RuntimeProviderHealthProbe`

## Why Not Split Catalog And Probe Yet

Those types are related to provider discovery and health checks, but they are already used as simple adapters by `models.rs`. Splitting them now would add another module boundary without reducing the main repository complexity much. The smallest safe win is to split the repository traits first.

## Compatibility Rules

- Keep all public constructors and names stable.
- Keep `models.rs` imports stable if possible.
- Preserve SQL behavior and error messages.
- Do not change routing, provider, or model behavior as part of this split.

## Test Strategy

Reuse the existing coverage in:

- `apps/runtime/src-tauri/tests/test_models.rs`
- `apps/runtime/src-tauri/tests/test_skill_route_settings.rs`

If any regression appears, add the narrowest focused test that proves the old behavior still works.

## Success Criteria

- `models_repo.rs` becomes materially thinner
- config-write and read-heavy logic live in separate child modules
- existing `models.rs` callers keep working without API churn
- Rust verification remains green
