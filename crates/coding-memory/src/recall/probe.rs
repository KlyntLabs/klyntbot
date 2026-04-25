//! C3 retrieval-quality probe.
//!
//! `coverage_score = mean(top_k.sim) - min(top_k.sim)`. Below threshold the
//! caller dispatches to the `RetrievalSkillRegistry`. Threshold defaults to
//! 0.3 but Phase 6's autotuner will train it.

/// Probe verdict — does retrieval need escalation?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// Coverage is acceptable.
    Sufficient,
    /// Coverage is below threshold — caller should escalate.
    Escalate,
}

/// Coverage probe with a configurable threshold.
#[derive(Debug, Clone, Copy)]
pub struct RetrievalQualityProbe {
    threshold: f32,
}

impl RetrievalQualityProbe {
    /// Construct.
    #[must_use]
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    /// Compute coverage_score.
    #[must_use]
    pub fn score(&self, sims: &[f32]) -> f32 {
        if sims.is_empty() {
            return 0.0;
        }
        let mean: f32 = sims.iter().sum::<f32>() / (sims.len() as f32);
        let min: f32 = sims.iter().cloned().fold(f32::INFINITY, f32::min);
        mean - min
    }

    /// Verdict given top-k similarities.
    #[must_use]
    pub fn verdict(&self, sims: &[f32]) -> ProbeVerdict {
        if sims.is_empty() || self.score(sims) < self.threshold {
            ProbeVerdict::Escalate
        } else {
            ProbeVerdict::Sufficient
        }
    }

    /// Active threshold.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }
}

impl Default for RetrievalQualityProbe {
    fn default() -> Self {
        Self::new(0.3)
    }
}
