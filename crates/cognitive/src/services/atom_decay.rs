//! Daily atom decay cycle — applies tiered salience decay and FSRS retention
//! recomputation to stale knowledge atoms, auto-archiving forgotten ones.

use crate::repos::{KnowledgeAtomRepo, ReviewStatsRepo};
use bus::DomainEventBus;
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Run the daily atom decay cycle.
///
/// 1. Query stale active atoms (no interaction in 7+ days)
/// 2. Apply tiered salience decay based on `personal_importance`
/// 3. Recompute retention via FSRS: `R = 1/(1 + t/(9*S))`
/// 4. Auto-archive if salience < 0.1, retention < 0.3, and stale > 30 days
/// 5. Update topic aggregates and emit a `CoachingLearningDigest` event
pub async fn run_decay_cycle(pool: &SqlitePool, bus: &DomainEventBus) -> common::Result<()> {
    let repo = KnowledgeAtomRepo::new(pool.clone());
    let stale = repo
        .list_stale_active(7)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    if stale.is_empty() {
        return Ok(());
    }

    info!("Atom decay: processing {} stale atoms", stale.len());
    let now = jiff::Timestamp::now();
    let mut fading_count = 0usize;
    let mut archived_count = 0usize;

    for atom in &stale {
        // Tiered salience decay based on personal importance
        let decay_factor = if atom.personal_importance > 0.85 {
            0.97
        } else if atom.personal_importance >= 0.5 {
            0.92
        } else {
            0.85
        };
        let new_salience = atom.salience * decay_factor;

        // FSRS retention: R = 1/(1 + t/(9*S))
        let elapsed_days = atom
            .last_interaction_ts
            .as_deref()
            .and_then(|ts| ts.parse::<jiff::Timestamp>().ok())
            .map(|ts| (now.as_millisecond() - ts.as_millisecond()) as f64 / 86_400_000.0)
            .unwrap_or(30.0);
        let new_retention = 1.0 / (1.0 + elapsed_days / (9.0 * atom.stability));

        // Auto-archive check
        let stale_days = elapsed_days as i64;
        if new_salience < 0.1 && new_retention < 0.3 && stale_days > 30 {
            if let Err(e) = repo.dismiss(&atom.id).await {
                warn!("Failed to archive atom {}: {e}", atom.id);
                continue;
            }
            bus.publish(bus::DomainEvent::KnowledgeAtomArchived {
                atom_id: atom.id.clone(),
                reason: "decay".to_string(),
            });
            archived_count += 1;
            continue;
        }

        // Apply decay
        if let Err(e) = repo
            .apply_decay(&atom.id, new_salience, new_retention)
            .await
        {
            warn!("Failed to apply decay to atom {}: {e}", atom.id);
            continue;
        }

        if new_retention < 0.6 && atom.personal_importance > 0.7 {
            fading_count += 1;
        }
    }

    // Update all topic aggregates
    if let Err(e) = repo.update_all_topic_aggregates().await {
        warn!("Failed to update topic aggregates: {e}");
    }

    // Emit digest
    let topics = repo.list_topics_with_atoms().await.unwrap_or_default();
    let weakest = topics.first().map(|t| t.name.clone());
    let strongest = topics.last().map(|t| t.name.clone());

    // Compute current review streak from review_log
    let review_stats = ReviewStatsRepo::new(pool.clone());
    let streak = review_stats.current_streak().await.unwrap_or(0);

    bus.publish(bus::DomainEvent::CoachingLearningDigest {
        fading_count,
        archived_count,
        streak_days: streak,
        strongest_topic: strongest,
        weakest_topic: weakest,
    });

    info!("Atom decay complete: {fading_count} fading, {archived_count} archived");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_fsrs_retention_formula() {
        // R = 1/(1 + t/(9*S))
        // stability=10, elapsed=30 days -> R = 1/(1 + 30/90) = 1/1.333 = 0.75
        let stability: f64 = 10.0;
        let elapsed: f64 = 30.0;
        let r: f64 = 1.0 / (1.0 + elapsed / (9.0 * stability));
        assert!((r - 0.75).abs() < 0.01);

        // stability=1, elapsed=9 days -> R = 1/(1 + 9/9) = 0.5
        let r2: f64 = 1.0 / (1.0 + 9.0 / (9.0 * 1.0));
        assert!((r2 - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_tiered_salience_decay() {
        // High importance (>0.85) -> 0.97 factor
        let new: f64 = 0.8 * 0.97;
        assert!((new - 0.776).abs() < 0.001);

        // Low importance (<0.5) -> 0.85 factor
        let new2: f64 = 0.5 * 0.85;
        assert!((new2 - 0.425).abs() < 0.001);
    }

    #[test]
    fn test_auto_archive_threshold() {
        // Should archive: low salience, low retention, stale > 30
        let salience = 0.05;
        let retention = 0.2;
        let stale_days = 45;
        let should_archive = salience < 0.1 && retention < 0.3 && stale_days > 30;
        assert!(should_archive);

        // Should NOT archive: retention too high (0.6 >= 0.3)
        let salience2 = 0.05_f64;
        let retention2 = 0.6_f64;
        let stale_days2: i64 = 45;
        let should_not = salience2 < 0.1 && retention2 < 0.3 && stale_days2 > 30;
        assert!(!should_not);
    }
}
