# Entity-Update Dispatch Discovery (v3 Task 30)

**Goal:** Locate every code path that emits an "entity changed" notification (UI refresh signal).

## Findings

### Dispatch site 1: `crates/desktop/src/commands/mod.rs:60`
- Signature: `pub fn emit_updates(app: &tauri::AppHandle, updates: &[::app_core::EntityUpdate])`
- Triggered by: Tauri command handlers after mutating operations
- Payload shape: `EntityUpdatedPayload { entity_kind: EntityKind, id: String }`
- Notes: This is the desktop-specific Tauri event emitter. It iterates over `EntityUpdate` structs and emits `entity:updated` events to the frontend.

### Dispatch site 2: Handler return values
- Handlers in `crates/app-core/src/handlers/` return `HandlerResult<T> = Result<(T, Vec<EntityUpdate>), ApiError>`
- The `EntityUpdate` struct is defined in `crates/app-core/src/state.rs:28`
- `EntityUpdate` uses `EntityKind` enum from `desktop_shared::types::EntityKind`
- Handlers hardcode the `EntityKind` variant: e.g., `EntityUpdate { kind: EntityKind::Task, id: id.clone() }`

### Files using the pattern
| File | EntityKind(s) used |
|---|---|
| `handlers/tasks/crud.rs` | `Task` |
| `handlers/projects.rs` | `Project` |
| `handlers/areas.rs` | `Area` |
| `handlers/notes/crud.rs` | `Note` |
| `handlers/notes/notebooks.rs` | `Notebook` |
| `handlers/objectives.rs` | `Objective` |
| `handlers/key_results.rs` | `KeyResult`, `Objective` |
| `handlers/project_sources.rs` | `Source` |
| `handlers/finance/mod.rs` | `Finance` |

### No server-side equivalent
- `crates/klyntbot-server/src/` has no `emit_updates` or `EntityUpdate` references
- The dev-server relies on the `AppEventEmitter` trait (`event_emitter.emit_entity_updated(...)`) which is called from the desktop layer

## Conclusion

The actual dispatch path is:
1. **Handler** returns `(data, Vec<EntityUpdate>)` where `EntityUpdate { kind: EntityKind, id }`
2. **Desktop command wrapper** unpacks the result and calls `emit_updates(app, &updates)`
3. **`emit_updates`** calls `emit_entity_updated(app, kind, id)` for each update
4. **`emit_entity_updated`** constructs `EntityUpdatedPayload` and emits a Tauri event

This differs from the plan's assumption of `emit_updates(&app, &updates)` calls inside handlers. The handlers use `EntityKind` enum (from `desktop_shared`), not string `kind` values.

**Adaptation for Tasks 49-50:**
- `mcp::dispatch_entity_update(kind, id)` returns `EntityUpdate { kind: String, id: String, domain: RecallDomain }` for MCP-side usage
- A separate bridge `registry_entity_kind_to_desktop(kind: &str) -> Option<EntityKind>` maps registry `entity_kind` strings to `desktop_shared::EntityKind` enum variants
- Full migration of all handlers would require passing `feature_registry` into every handler or making `EntityKind` derivation registry-driven — this is deferred to v3.x as it touches ~15 handler files
