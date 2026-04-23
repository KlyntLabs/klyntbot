//! Auto-derived entity-update dispatch for MCP entity-changed notifications.
//! The lookup table is sourced from `ai_core::AiFeatureRegistry::iter()`.

use ai_core::{AiFeatureRegistry, FeatureRecord, RecallDomain};

/// Identifies an entity update by kind + ID. Maps 1:1 to the prior
/// hand-coded `emit_updates(&app, &updates)` payload shape (see
/// docs/superpowers/notes/2026-04-23-entity-update-discovery.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUpdate {
    pub kind: String,
    pub id: String,
    pub domain: RecallDomain,
}

/// Resolve an `(entity_kind, id)` into an `EntityUpdate` carrying the right
/// `RecallDomain`. Returns `None` if no feature owns this kind — caller
/// decides whether to log+drop or panic.
pub fn dispatch_entity_update(
    reg: &AiFeatureRegistry,
    kind: &str,
    id: &str,
) -> Option<EntityUpdate> {
    let rec: &FeatureRecord = reg.by_entity_kind(kind)?;
    Some(EntityUpdate {
        kind: kind.to_string(),
        id: id.to_string(),
        domain: rec.domain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::FeatureRecord;

    fn fake_registry() -> AiFeatureRegistry {
        let mut reg = AiFeatureRegistry::new();
        reg.register(FeatureRecord {
            domain: RecallDomain::Tasks,
            skill: "task-management",
            tool_name: Some("tasks"),
            entity_kind: Some("task"),
        });
        reg
    }

    #[test]
    fn dispatch_returns_some_for_known_kind() {
        let reg = fake_registry();
        let upd = dispatch_entity_update(&reg, "task", "abc").expect("Some");
        assert_eq!(upd.kind, "task");
        assert_eq!(upd.id, "abc");
        assert_eq!(upd.domain, RecallDomain::Tasks);
    }

    #[test]
    fn dispatch_returns_none_for_unknown_kind() {
        let reg = fake_registry();
        assert!(dispatch_entity_update(&reg, "unknown_kind", "x").is_none());
    }
}
