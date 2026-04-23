//! AiFeatureRegistry — runtime collection of FeatureRecord entries built at
//! app-core startup. Consumed by:
//! - `default_exposed_tools()` (MCP server tool exposure)
//! - `dispatch_entity_update()` (MCP entity-update fan-out)
//! - `RecallProviderRegistry` seeding (cognitive context source)
//! - the activity-log normalizer consumer (event-kind allowlist)

use crate::RecallDomain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureRecord {
    pub domain: RecallDomain,
    pub skill: &'static str,
    pub tool_name: Option<&'static str>,
    pub entity_kind: Option<&'static str>,
}

#[derive(Debug, Default)]
pub struct AiFeatureRegistry {
    records: Vec<FeatureRecord>,
}

impl AiFeatureRegistry {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Register a feature. Panics if the same `RecallDomain` is registered twice
    /// — this catches accidental double-registration in `app-core::init`.
    pub fn register(&mut self, record: FeatureRecord) {
        if self.records.iter().any(|r| r.domain == record.domain) {
            panic!(
                "AiFeatureRegistry: duplicate registration for domain {:?}",
                record.domain
            );
        }
        self.records.push(record);
    }

    pub fn by_domain(&self, domain: &RecallDomain) -> Option<&FeatureRecord> {
        self.records.iter().find(|r| &r.domain == domain)
    }

    pub fn by_entity_kind(&self, kind: &str) -> Option<&FeatureRecord> {
        self.records.iter().find(|r| r.entity_kind == Some(kind))
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        self.records.iter().filter_map(|r| r.tool_name).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FeatureRecord> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
