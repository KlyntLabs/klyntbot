//! Progress computation for insight reviews.
//!
//! Computes 4 learning signals:
//! 1. Flashcard success (from FlashcardAccessor — weight 0.40)
//! 2. Semantic drift (from InsightEmbedder — weight 0.25)
//! 3. Gap closure (from content comparison — weight 0.20)
//! 4. Quiz score (from self-assessment — weight 0.15)

use std::sync::Arc;

use crate::progress_repo::InsightProgressRepo;
use crate::repo::InsightReviewRepo;
use crate::traits::{FlashcardAccessor, InsightEmbedder};
use crate::types::{InsightContent, InsightReviewRow, ProgressSnapshotRow, ProgressWeights};

/// Computes learning progress for insight reviews.
pub struct ProgressComputer {
    repo: InsightReviewRepo,
    progress_repo: InsightProgressRepo,
    flashcards: Arc<dyn FlashcardAccessor>,
    embedder: Arc<dyn InsightEmbedder>,
    weights: ProgressWeights,
}

impl ProgressComputer {
    pub fn new(
        repo: InsightReviewRepo,
        progress_repo: InsightProgressRepo,
        flashcards: Arc<dyn FlashcardAccessor>,
        embedder: Arc<dyn InsightEmbedder>,
        weights: ProgressWeights,
    ) -> Self {
        Self {
            repo,
            progress_repo,
            flashcards,
            embedder,
            weights,
        }
    }

    /// Compute and store a progress snapshot for a specific insight.
    ///
    /// Gathers all 4 signals and stores the result via InsightProgressRepo.
    ///
    /// `quiz_score_override` — if `Some`, uses the provided score instead of
    /// the default 0.0. Called with a real score after the user completes a quiz.
    pub async fn compute(
        &self,
        insight: &InsightReviewRow,
        note_body: &str,
        quiz_score_override: Option<f64>,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        // Fetch versions once for both drift and gap closure
        let prev = if insight.version > 1 {
            self.repo
                .list_versions(&insight.note_id)
                .await
                .ok()
                .and_then(|versions| {
                    versions
                        .into_iter()
                        .find(|v| v.version == insight.version - 1)
                })
        } else {
            None
        };

        // 1. Flashcard success (from FSRS metrics)
        let flashcard_success = self.flashcards.review_success_rate(&insight.id, 30).await;

        // 2. Semantic drift (cosine distance from previous version)
        let semantic_drift = self.compute_semantic_drift(insight, prev.as_ref()).await;

        // 3. Gap closure (compare previous gaps against current note body)
        let gap_closure = Self::compute_gap_closure(prev.as_ref(), note_body);

        // 4. Quiz score (from user quiz answers, or 0.0 if no quiz data yet)
        let quiz_score = quiz_score_override.unwrap_or(0.0);

        self.progress_repo
            .upsert(
                &insight.id,
                insight.version,
                flashcard_success,
                semantic_drift,
                gap_closure,
                quiz_score,
                &self.weights,
            )
            .await
    }

    /// Compute semantic drift between this version and the previous one.
    ///
    /// Returns 0.0 for v1 (no previous version) or when embeddings are unavailable.
    async fn compute_semantic_drift(
        &self,
        insight: &InsightReviewRow,
        prev: Option<&InsightReviewRow>,
    ) -> f64 {
        let Some(prev) = prev else {
            return 0.0;
        };

        // Cosine similarity → drift = 1.0 - similarity
        match self.embedder.similarity(&insight.id, &prev.id).await {
            Some(sim) => (1.0 - sim).clamp(0.0, 1.0),
            None => 0.0, // Embeddings not available
        }
    }

    /// Compute gap closure: what fraction of previous version's gaps are addressed.
    ///
    /// Parses gap bullets from previous version's gap_analysis, checks how many
    /// topic keywords appear in the current note body (case-insensitive substring match).
    fn compute_gap_closure(prev: Option<&InsightReviewRow>, note_body: &str) -> f64 {
        let Some(prev) = prev else {
            return 0.0;
        };

        let prev_content: InsightContent = serde_json::from_str(&prev.content).unwrap_or_default();

        let Some(ref gaps_text) = prev_content.gap_analysis else {
            return 0.0;
        };

        let gap_topics = extract_gap_topics(gaps_text);
        if gap_topics.is_empty() {
            return 0.0;
        }

        let body_lower = note_body.to_lowercase();
        let closed = gap_topics
            .iter()
            .filter(|topic| body_lower.contains(&topic.to_lowercase()))
            .count();

        closed as f64 / gap_topics.len() as f64
    }
}

/// Extract topic keywords from gap analysis text.
///
/// Looks for lines starting with `- **` (markdown bold bullet) and extracts
/// the bold text as the topic. Falls back to extracting first words after `- `.
fn extract_gap_topics(gaps_text: &str) -> Vec<String> {
    let mut topics = Vec::new();
    for line in gaps_text.lines() {
        let trimmed = line.trim();
        // Pattern: "- **Topic Name** — description"
        if let Some(rest) = trimmed.strip_prefix("- **") {
            if let Some(end) = rest.find("**") {
                let topic = rest[..end].trim().to_string();
                if !topic.is_empty() {
                    topics.push(topic);
                }
                continue;
            }
        }
        // Pattern: "- Topic text here"
        if let Some(rest) = trimmed.strip_prefix("- ") {
            // Take first 3 words as the topic
            let topic: String = rest
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            if !topic.is_empty() && topic.len() > 3 {
                topics.push(topic);
            }
        }
    }
    topics
}

/// Generate a human-readable change note comparing two progress snapshots.
pub fn generate_change_note(
    current: &ProgressSnapshotRow,
    previous: Option<&ProgressSnapshotRow>,
) -> String {
    let Some(prev) = previous else {
        return "Initial insight generated".to_string();
    };
    let delta = current.overall_progress - prev.overall_progress;
    let direction = if delta >= 0.0 { "Improved" } else { "Declined" };
    let pct = (delta.abs() * 100.0).round() as i32;

    if pct == 0 {
        return "No significant change".to_string();
    }

    let signals = [
        (
            "flashcard reviews",
            current.flashcard_success - prev.flashcard_success,
        ),
        (
            "content stability",
            (1.0 - current.semantic_drift) - (1.0 - prev.semantic_drift),
        ),
        ("gap closure", current.gap_closure - prev.gap_closure),
        ("quiz performance", current.quiz_score - prev.quiz_score),
    ];
    let best = signals
        .iter()
        .max_by(|a, b| {
            a.1.abs()
                .partial_cmp(&b.1.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    format!("{direction} {pct}% — driven by {}", best.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_gap_topics_bold_bullets() {
        let gaps = "## Gaps\n\
            - **Distributed consensus** — not covered\n\
            - **CAP theorem** — only mentioned briefly\n\
            Some other text";
        let topics = extract_gap_topics(gaps);
        assert_eq!(topics, vec!["Distributed consensus", "CAP theorem"]);
    }

    #[test]
    fn test_extract_gap_topics_plain_bullets() {
        let gaps = "- Missing coverage of event sourcing patterns\n\
            - Shallow treatment of CQRS";
        let topics = extract_gap_topics(gaps);
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0], "Missing coverage of");
        assert_eq!(topics[1], "Shallow treatment of");
    }

    #[test]
    fn test_extract_gap_topics_empty() {
        assert!(extract_gap_topics("No bullet points here").is_empty());
        assert!(extract_gap_topics("").is_empty());
    }

    #[test]
    fn test_change_note_initial() {
        let current = sample_snapshot(0.25, 0.0, 0.0, 0.0, 0.25);
        assert_eq!(
            generate_change_note(&current, None),
            "Initial insight generated"
        );
    }

    #[test]
    fn test_change_note_improved() {
        let prev = sample_snapshot(0.2, 0.0, 0.0, 0.0, 0.33);
        let current = sample_snapshot(0.8, 0.1, 0.5, 0.0, 0.63);
        let note = generate_change_note(&current, Some(&prev));
        assert!(note.starts_with("Improved"));
        assert!(note.contains("flashcard reviews"));
    }

    #[test]
    fn test_change_note_no_change() {
        let snap = sample_snapshot(0.5, 0.0, 0.0, 0.0, 0.45);
        assert_eq!(
            generate_change_note(&snap, Some(&snap)),
            "No significant change"
        );
    }

    fn sample_snapshot(
        flashcard: f64,
        drift: f64,
        gap: f64,
        quiz: f64,
        overall: f64,
    ) -> ProgressSnapshotRow {
        ProgressSnapshotRow {
            id: "snap-1".to_string(),
            insight_review_id: "insight-1".to_string(),
            version: 1,
            flashcard_success: flashcard,
            semantic_drift: drift,
            gap_closure: gap,
            quiz_score: quiz,
            overall_progress: overall,
            computed_at: "2026-03-17T00:00:00Z".to_string(),
        }
    }
}
