//! AutoTuner orchestrator — thin L5 glue wiring the autotuner crate to the
//! agent runtime.  Holds champion state, coordinates shadow classification,
//! and metric collection.

pub mod hooks;
pub mod metric_collector;
pub mod shadow_classifier;

use autotuner::{Champion, ChampionSummary};
use common::TrialParams;
use tokio::sync::RwLock;

/// Thin glue that holds the current champion state and exposes it to the
/// agent runtime.  The nightly cycle (Task 15) updates the champion via
/// [`update_champion`].
pub struct AutoTunerOrchestrator {
    champion: RwLock<Champion>,
    active: bool,
}

impl AutoTunerOrchestrator {
    pub fn new(champion: Champion, enabled: bool) -> Self {
        Self {
            champion: RwLock::new(champion),
            active: enabled,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Return the current champion's trial params if the autotuner is active
    /// and a non-default champion has been promoted.
    pub async fn current_champion_params(&self) -> Option<TrialParams> {
        if !self.active {
            return None;
        }
        let champion = self.champion.read().await;
        if champion.trial_id.is_some() {
            Some(champion.params.clone())
        } else {
            None
        }
    }

    /// Replace the champion after a successful promotion.
    pub async fn update_champion(&self, new_champion: Champion) {
        *self.champion.write().await = new_champion;
    }

    /// Build a summary for the transparency panel / events.
    pub async fn champion_summary(&self) -> ChampionSummary {
        let c = self.champion.read().await;
        let days = (chrono::Utc::now() - c.promoted_at).num_days().max(0) as u32;
        ChampionSummary {
            trial_id: c.trial_id,
            description: c.reason_for_promotion.clone(),
            impact: c.impact_summary.clone(),
            promoted_at: c.promoted_at,
            days_active: days,
        }
    }

    /// Clone the full champion (for nightly cycle evaluation).
    pub async fn champion(&self) -> Champion {
        self.champion.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn inactive_returns_no_params() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), false);
        assert!(!orch.is_active());
        assert!(orch.current_champion_params().await.is_none());
    }

    #[tokio::test]
    async fn active_default_champion_returns_none() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), true);
        assert!(orch.is_active());
        // Default champion has trial_id = None → returns None.
        assert!(orch.current_champion_params().await.is_none());
    }

    #[tokio::test]
    async fn active_promoted_champion_returns_params() {
        let champion = Champion {
            trial_id: Some(uuid::Uuid::new_v4()),
            params: TrialParams {
                heuristic_confidence_threshold: Some(0.75),
                ..Default::default()
            },
            ..Champion::default()
        };
        let orch = AutoTunerOrchestrator::new(champion, true);
        let params = orch.current_champion_params().await;
        assert!(params.is_some());
        assert_eq!(params.unwrap().heuristic_confidence_threshold, Some(0.75));
    }

    #[tokio::test]
    async fn update_champion_replaces_state() {
        let orch = AutoTunerOrchestrator::new(Champion::default(), true);
        assert!(orch.champion().await.trial_id.is_none());

        let new = Champion {
            trial_id: Some(uuid::Uuid::new_v4()),
            reason_for_promotion: "improved accuracy".into(),
            ..Champion::default()
        };
        orch.update_champion(new.clone()).await;

        let current = orch.champion().await;
        assert!(current.trial_id.is_some());
        assert_eq!(current.reason_for_promotion, "improved accuracy");
    }

    #[tokio::test]
    async fn champion_summary_populates_days_active() {
        let champion = Champion {
            promoted_at: chrono::Utc::now() - chrono::Duration::days(3),
            ..Champion::default()
        };
        let orch = AutoTunerOrchestrator::new(champion, true);
        let summary = orch.champion_summary().await;
        assert!(summary.days_active >= 3);
    }
}
