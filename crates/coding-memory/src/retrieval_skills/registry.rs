//! Registry — owns the closed set of 5 skills, tracks per-skill EMA, runs the
//! tier-aware selector. Effectiveness updates land via
//! `DomainEvent::RetrievalSkillApplied`.

use super::{BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill};
use bus::{DomainEvent, DomainEventBus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result of running the selector.
#[derive(Debug, Clone)]
pub struct SelectorOutcome {
    /// Names of skills that ran (in the order tried).
    pub skills_tried: Vec<String>,
    /// Final outcome (last successful skill, or last failure).
    pub final_outcome: EscalationOutcome,
}

/// Registry — built once at AppCore boot.
pub struct RetrievalSkillRegistry {
    skills: Vec<Arc<dyn RetrievalSkill>>,
    effectiveness: RwLock<HashMap<String, f32>>,
    bus: Arc<DomainEventBus>,
}

impl std::fmt::Debug for RetrievalSkillRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetrievalSkillRegistry")
            .field("skill_count", &self.skills.len())
            .finish()
    }
}

impl RetrievalSkillRegistry {
    /// Construct with explicit skill list (test seam).
    pub fn new(skills: Vec<Arc<dyn RetrievalSkill>>, bus: Arc<DomainEventBus>) -> Self {
        let mut eff = HashMap::new();
        for s in &skills {
            eff.insert(s.name().to_string(), 0.5);
        }
        Self {
            skills,
            effectiveness: RwLock::new(eff),
            bus,
        }
    }

    /// Read the current EMA score for a skill.
    pub async fn effectiveness_of(&self, name: &str) -> f32 {
        self.effectiveness
            .read()
            .await
            .get(name)
            .copied()
            .unwrap_or(0.5)
    }

    /// Update EMA after observing `outcome_value` (1.0 success, 0.0 failure, 0.5 partial).
    pub async fn record_outcome(&self, name: &str, outcome_value: f32) {
        let mut w = self.effectiveness.write().await;
        let prev = w.get(name).copied().unwrap_or(0.5);
        let next = 0.9 * prev + 0.1 * outcome_value;
        w.insert(name.to_string(), next);
    }

    /// Run the selector — try skills in highest-EMA order within the active tier.
    /// Stops on first success; otherwise returns the final failed outcome.
    pub async fn escalate(&self, ctx: &EscalationContext) -> common::Result<SelectorOutcome> {
        // Pick candidates: skills whose tier <= active tier.
        let active = ctx.budget_tier;
        let mut candidates: Vec<Arc<dyn RetrievalSkill>> = self
            .skills
            .iter()
            .filter(|s| tier_rank(s.tier()) <= tier_rank(active))
            .cloned()
            .collect();

        // Sort by EMA descending.
        let eff = self.effectiveness.read().await.clone();
        candidates.sort_by(|a, b| {
            let ae = eff.get(a.name()).copied().unwrap_or(0.5);
            let be = eff.get(b.name()).copied().unwrap_or(0.5);
            be.partial_cmp(&ae).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut tried: Vec<String> = Vec::new();
        let mut last: Option<EscalationOutcome> = None;
        for skill in candidates {
            let name = skill.name().to_string();
            tried.push(name.clone());
            let before = ctx.coverage_score;
            let out = skill.apply(ctx).await?;
            let after = out.coverage_after;
            self.bus.publish(DomainEvent::RetrievalSkillApplied {
                skill: name.clone(),
                before_score: before,
                after_score: after,
                budget_used: format!("{:?}", skill.tier()),
                session_id: String::new(),
            });
            let outcome_value = if out.succeeded { 1.0 } else { 0.0 };
            self.record_outcome(&name, outcome_value).await;
            if out.succeeded {
                last = Some(out);
                break;
            }
            last = Some(out);
        }
        Ok(SelectorOutcome {
            skills_tried: tried,
            final_outcome: last.unwrap_or(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            }),
        })
    }
}

fn tier_rank(t: BudgetTier) -> u8 {
    match t {
        BudgetTier::Fast => 0,
        BudgetTier::DeepThink => 1,
        BudgetTier::Ultra => 2,
    }
}
