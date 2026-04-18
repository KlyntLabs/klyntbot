//! A deterministic [`NarrativeHandler`] that produces template-based mirror
//! narratives from context data without any LLM calls.

use async_trait::async_trait;

use cognitive::mirror::{GeneratedNarrative, NarrativeContext, NarrativeHandler};
use common::Result;

/// A [`NarrativeHandler`] that generates narratives via pure string formatting.
///
/// Useful in simulation runs where deterministic, reproducible output is
/// required and no external LLM provider should be invoked.
pub struct HeuristicNarrativeHandler;

#[async_trait]
impl NarrativeHandler for HeuristicNarrativeHandler {
    async fn generate_narrative(&self, ctx: NarrativeContext) -> Result<GeneratedNarrative> {
        let snapshot_count = ctx.routing_snapshots.len();

        // Format top 3 skills by usage as "name (X%)"
        let top_skills: Vec<String> = ctx
            .top_skills_by_usage
            .iter()
            .take(3)
            .map(|(name, pct)| format!("{name} ({pct:.0}%)"))
            .collect();

        let skills_summary = if top_skills.is_empty() {
            "no skill usage recorded".to_string()
        } else {
            top_skills.join(", ")
        };

        let routing_summary = format!(
            "Over {snapshot_count} routing snapshot(s), top skills were: {skills_summary}. \
             Corrections: {}.",
            ctx.correction_count,
        );

        let full_narrative = format!(
            "This period covered {snapshot_count} snapshot(s) with {correction} correction(s). \
             The most-used skills were {skills_summary}.",
            correction = ctx.correction_count,
        );

        let improvement_highlights = if ctx.correction_count == 0 {
            vec!["No corrections recorded — routing appears stable.".to_string()]
        } else {
            vec![format!(
                "{} correction(s) detected; consider reviewing routing preferences.",
                ctx.correction_count,
            )]
        };

        Ok(GeneratedNarrative {
            full_narrative,
            routing_summary,
            improvement_highlights,
        })
    }

    async fn generate_mirror_response(&self, query: &str, ctx: NarrativeContext) -> Result<String> {
        let snapshot_count = ctx.routing_snapshots.len();
        let skill_count = ctx.top_skills_by_usage.len();

        Ok(format!(
            "Mirror response for \"{query}\": {snapshot_count} snapshot(s), \
             {skill_count} skill(s) tracked.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context(skills: Vec<(&str, f64)>, corrections: u32) -> NarrativeContext {
        NarrativeContext {
            period: (jiff::Timestamp::now(), jiff::Timestamp::now()),
            routing_snapshots: vec![],
            correction_count: corrections,
            top_skills_by_usage: skills
                .into_iter()
                .map(|(s, p)| (s.to_string(), p))
                .collect(),
            past_narrative_feedback: vec![],
        }
    }

    #[tokio::test]
    async fn generates_narrative_with_skills() {
        let ctx = sample_context(
            vec![("general", 55.0), ("finance", 30.0), ("tasks", 15.0)],
            2,
        );
        let handler = HeuristicNarrativeHandler;
        let result = handler.generate_narrative(ctx).await.unwrap();

        assert!(result.routing_summary.contains("general (55%)"));
        assert!(result.routing_summary.contains("finance (30%)"));
        assert!(result.routing_summary.contains("tasks (15%)"));
        assert!(result.full_narrative.contains("2 correction(s)"));
        assert!(!result.improvement_highlights.is_empty());
    }

    #[tokio::test]
    async fn generates_narrative_no_skills() {
        let ctx = sample_context(vec![], 0);
        let handler = HeuristicNarrativeHandler;
        let result = handler.generate_narrative(ctx).await.unwrap();

        assert!(result.routing_summary.contains("no skill usage recorded"));
        assert!(
            result.improvement_highlights[0].contains("stable"),
            "Expected stable highlight, got: {}",
            result.improvement_highlights[0]
        );
    }

    #[tokio::test]
    async fn generates_mirror_response() {
        let ctx = sample_context(vec![("general", 80.0), ("tasks", 20.0)], 1);
        let handler = HeuristicNarrativeHandler;
        let result = handler
            .generate_mirror_response("how am I doing?", ctx)
            .await
            .unwrap();

        assert!(result.contains("how am I doing?"));
        assert!(result.contains("0 snapshot(s)"));
        assert!(result.contains("2 skill(s)"));
    }

    #[tokio::test]
    async fn truncates_to_top_three_skills() {
        let ctx = sample_context(vec![("a", 40.0), ("b", 30.0), ("c", 20.0), ("d", 10.0)], 0);
        let handler = HeuristicNarrativeHandler;
        let result = handler.generate_narrative(ctx).await.unwrap();

        assert!(result.routing_summary.contains("a (40%)"));
        assert!(result.routing_summary.contains("b (30%)"));
        assert!(result.routing_summary.contains("c (20%)"));
        assert!(!result.routing_summary.contains("d (10%)"));
    }
}
