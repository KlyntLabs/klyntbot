//! Finance handlers — read-only queries + mutations against FinanceStorage repos.

mod accounts;
mod budgets;
mod investments;
mod reports;
mod transactions;

use desktop_shared::types::EntityKind;

use crate::state::{AppCore, EntityUpdate};

impl AppCore {
    // ── Helpers ──────────────────────────────────────────────────

    /// Read the configured default currency (e.g. "USD", "VND").
    pub(crate) async fn default_currency(&self) -> String {
        self.config.read().await.finance.default_currency.clone()
    }

    /// Build the entity-update vec common to all finance mutations.
    pub(crate) fn finance_updates(id: String) -> Vec<EntityUpdate> {
        vec![EntityUpdate {
            kind: EntityKind::Finance,
            id,
        }]
    }
}
