# src-tauri Phase 3A Models Application Layer Design

## Goal

Extract the configuration and routing use cases from `apps/runtime/src-tauri/src/commands/models.rs` into a dedicated application-layer crate so the dependency direction becomes:

- Tauri command adapters depend on application services
- application services depend on traits
- `src-tauri` infrastructure implements those traits

This phase does not redesign chat orchestration, health probing, or bootstrap wiring.

## Why Phase 3A

Phase 1 and Phase 2 already removed several pure-logic clusters from `src-tauri`:

- skill registry/config parsing
- permission policy logic
- routing/model pure rules
- executor pure helpers

The next structural bottleneck is `commands/models.rs`. It still mixes:

- Tauri command surface
- SQLx access
- provider registry access
- DTO definitions
- use-case orchestration
- route template application

That file is now less of a rules module and more of a mixed adapter/application/infrastructure layer. The next healthy step is to separate those responsibilities.

## Recommended Direction

Create a new crate:

- `packages/runtime-models-app`

This crate will own the models/routing configuration use cases. It will not depend on:

- Tauri
- SQLx
- `State<T>`
- `AppHandle`

It will depend on:

- plain Rust types
- trait-based repositories/catalogs
- `runtime-routing-core` for pure route rules

## Architectural Boundary

### Keep in `apps/runtime/src-tauri/src/commands/models.rs`

- `#[tauri::command]` functions
- request/response DTO mapping specific to command entrypoints
- `State<DbState>` and `State<ProviderRegistry>` extraction
- wiring application services to concrete adapters

### Move to `packages/runtime-models-app`

- routing settings use cases
- provider config use cases
- capability route template application use case
- provider plugin listing use case
- shared application-level result models

### Keep in `src-tauri` infrastructure for now

- SQLx queries and persistence
- `ProviderRegistry` access
- provider health probing
- route-attempt analytics queries
- any code that is tightly coupled to runtime state or external I/O

## Crate Design

Suggested file layout:

- `packages/runtime-models-app/Cargo.toml`
- `packages/runtime-models-app/src/lib.rs`
- `packages/runtime-models-app/src/types.rs`
- `packages/runtime-models-app/src/traits.rs`
- `packages/runtime-models-app/src/service.rs`
- `packages/runtime-models-app/tests/...`

## Trait Boundaries

The new crate should define narrow traits instead of depending on `SqlitePool` or `ProviderRegistry` directly.

### `ModelsConfigRepository`

Responsibilities:

- load routing settings
- save routing settings
- save provider config
- list provider configs
- delete provider config if supported
- persist capability routing policy produced by template application

### `ModelsReadRepository`

Responsibilities:

- read-only config/query access needed by app services
- enabled provider lookup for route template application
- lightweight model/routing views as needed by migrated use cases

This trait should stay minimal in Phase 3A. Do not add health/stat/reporting concerns unless strictly required.

### `ProviderCatalog`

Responsibilities:

- list provider plugins
- expose provider capability metadata

This trait exists so application services can transform runtime provider metadata into stable application DTOs without touching `ProviderRegistry` directly.

## Service Surface

The application layer should expose a service roughly shaped like:

- `ModelsAppService<R, C>`

Where:

- `R` implements the repository traits
- `C` implements `ProviderCatalog`

Expected use cases for Phase 3A:

- `load_routing_settings`
- `save_routing_settings`
- `save_provider_config`
- `list_provider_configs`
- `apply_capability_route_template`
- `list_provider_plugins`

## Scope Control

Phase 3A must stay intentionally narrow.

### In Scope

- configuration domain use cases
- route template application orchestration
- app-service DTOs and trait contracts
- thin `src-tauri` repo adapters

### Out of Scope

- provider health probe orchestration
- route attempt log/stat aggregation redesign
- chat/session orchestration
- bootstrap refactors
- storage crate extraction
- cross-command transaction abstractions

## Migration Strategy

### Step 1

Create `runtime-models-app` with types, traits, and tests using fake repositories/catalogs.

### Step 2

Implement thin infrastructure adapters inside `src-tauri`, for example:

- `apps/runtime/src-tauri/src/commands/models_repo.rs`

These adapters can keep using SQLx and `ProviderRegistry`.

### Step 3

Migrate the cleanest use case first:

- routing settings load/save

### Step 4

Migrate provider config CRUD use cases.

### Step 5

Migrate capability route template application, reusing `runtime-routing-core`.

### Step 6

Shrink `commands/models.rs` so it becomes a command adapter layer.

## Testing Strategy

### Application-layer tests

`runtime-models-app` should be the main testing surface.

Use fake repositories/catalogs to verify:

- routing settings defaults and persistence behavior
- provider config save/list behavior
- route template application against enabled providers
- provider plugin metadata mapping

### Minimal app-level tests

Keep only narrow integration tests in `src-tauri`:

- command-to-service wiring
- adapter mapping correctness
- a small number of smoke tests for affected commands

Do not rebuild a large `src-tauri` test matrix for application behavior that now belongs in the lightweight crate.

## Definition of Done

- `runtime-models-app` exists
- configuration-domain use cases move out of `commands/models.rs`
- `commands/models.rs` becomes materially thinner
- `src-tauri` implements traits instead of owning orchestration directly
- lightweight tests become the primary verification path for the migrated logic
- no spillover into chat/bootstrap redesign in this phase
