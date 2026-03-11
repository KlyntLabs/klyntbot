//! Tracks what context is loaded vs. available but not yet loaded.

use serde::{Deserialize, Serialize};

/// Status of a context source in the inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContextItemStatus {
    /// Source content has been loaded into the prompt.
    Loaded { tokens_used: usize },
    /// Source exists but was deferred due to budget constraints.
    Deferred { reason: String },
    /// Source is available but wasn't queried in this assembly.
    Available { description: String },
}

/// A single entry in the context inventory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextInventoryItem {
    pub source_name: String,
    pub priority: u8,
    pub status: ContextItemStatus,
    pub token_estimate: usize,
    pub summary: Option<String>,
}

/// Tracks all context sources and their load status.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextInventory {
    items: Vec<ContextInventoryItem>,
}

impl ContextInventory {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Read access to inventory items.
    pub fn items(&self) -> &[ContextInventoryItem] {
        &self.items
    }

    /// Add or update an inventory item.
    pub fn upsert(&mut self, item: ContextInventoryItem) {
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|i| i.source_name == item.source_name)
        {
            *existing = item;
        } else {
            self.items.push(item);
        }
    }

    /// Get total tokens used by loaded sources.
    pub fn tokens_loaded(&self) -> usize {
        self.items
            .iter()
            .map(|i| match &i.status {
                ContextItemStatus::Loaded { tokens_used } => *tokens_used,
                _ => 0,
            })
            .sum()
    }

    /// Check if any sources are deferred (no allocation).
    pub fn has_deferred(&self) -> bool {
        self.items
            .iter()
            .any(|i| matches!(&i.status, ContextItemStatus::Deferred { .. }))
    }

    /// Get names of deferred sources.
    pub fn deferred_sources(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter_map(|i| match &i.status {
                ContextItemStatus::Deferred { .. } => Some(i.source_name.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Mark a source as loaded.
    pub fn mark_loaded(&mut self, source_name: &str, tokens_used: usize) {
        if let Some(item) = self.items.iter_mut().find(|i| i.source_name == source_name) {
            item.status = ContextItemStatus::Loaded { tokens_used };
        }
    }

    /// Format as a human-readable summary for the system prompt.
    pub fn format_for_prompt(&self, budget_total: usize, budget_remaining: usize) -> String {
        let mut lines =
            vec!["[Available Context — request via context_request tool if needed]".to_string()];

        for item in &self.items {
            let status_icon = match &item.status {
                ContextItemStatus::Loaded { tokens_used } => {
                    format!("loaded — {:.1}k tokens", *tokens_used as f64 / 1000.0)
                }
                ContextItemStatus::Deferred { reason } => {
                    format!(
                        "deferred — {} ({:.1}k est.)",
                        reason,
                        item.token_estimate as f64 / 1000.0
                    )
                }
                ContextItemStatus::Available { description } => {
                    format!("available — {}", description)
                }
            };
            lines.push(format!("- {} ({})", item.source_name, status_icon));
        }

        lines.push(format!(
            "Budget: {:.1}k / {:.1}k tokens remaining",
            budget_remaining as f64 / 1000.0,
            budget_total as f64 / 1000.0,
        ));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_upsert_and_tokens() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "memories".into(),
            priority: 70,
            status: ContextItemStatus::Loaded { tokens_used: 1800 },
            token_estimate: 2000,
            summary: Some("Episodic memories".into()),
        });
        inv.upsert(ContextInventoryItem {
            source_name: "project".into(),
            priority: 50,
            status: ContextItemStatus::Deferred {
                reason: "budget insufficient".into(),
            },
            token_estimate: 2100,
            summary: Some("Project details".into()),
        });

        assert_eq!(inv.tokens_loaded(), 1800);
        assert_eq!(inv.deferred_sources(), vec!["project"]);
    }

    #[test]
    fn test_mark_loaded() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "project".into(),
            priority: 50,
            status: ContextItemStatus::Deferred {
                reason: "budget".into(),
            },
            token_estimate: 2100,
            summary: None,
        });

        inv.mark_loaded("project", 1900);
        assert_eq!(inv.tokens_loaded(), 1900);
        assert!(inv.deferred_sources().is_empty());
    }

    #[test]
    fn test_format_for_prompt() {
        let mut inv = ContextInventory::new();
        inv.upsert(ContextInventoryItem {
            source_name: "memories".into(),
            priority: 70,
            status: ContextItemStatus::Loaded { tokens_used: 1800 },
            token_estimate: 2000,
            summary: None,
        });

        let prompt = inv.format_for_prompt(12000, 8200);
        assert!(prompt.contains("memories"));
        assert!(prompt.contains("loaded"));
        assert!(prompt.contains("Budget"));
    }
}
