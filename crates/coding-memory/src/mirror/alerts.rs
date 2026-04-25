//! Closed-enum coding alert kinds + severity. Filled in by Task 18.

/// Closed enum of coding-specific Mirror alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodingMirrorAlertKind {
    /// Skill activation dropped > 50 % in 7 days.
    ProjectSkillObsolete,
    /// `problem_hash` recurred ≥ 3× without matching `WorkflowPattern`.
    UncapturedPattern,
    /// Global fact retrieved heavily in repo sessions, irrelevant.
    ScopeMisclassified,
    /// User edited managed block; auto-rewrite skipped.
    SkillFileConflict,
    /// Same `problem_hash` failed ≥ 3× across sessions.
    ProblemClassRefactor,
    /// User overrode ≥ 3 dead-end warnings in same repo.
    LowerDeadEndThreshold,
    /// Preference observed across ≥ 3 repos with low refutation.
    PromoteToGlobal,
    /// Counterfactual retrieved ≥ 5× and always ignored.
    PromoteCounterfactualVisibility,
    /// C2 git invalidation produced a `stale_candidate`.
    StaleFactDetected,
    /// Safety-net — a fact lacks valid provenance.
    ProvenanceMissing,
    /// > 50 pending distillations — LLM provider probably down.
    DistillerQueueBacklog,
}

impl CodingMirrorAlertKind {
    /// Lossless string form for persistence.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProjectSkillObsolete => "project_skill_obsolete",
            Self::UncapturedPattern => "uncaptured_pattern",
            Self::ScopeMisclassified => "scope_misclassified",
            Self::SkillFileConflict => "skill_file_conflict",
            Self::ProblemClassRefactor => "problem_class_refactor",
            Self::LowerDeadEndThreshold => "lower_dead_end_threshold",
            Self::PromoteToGlobal => "promote_to_global",
            Self::PromoteCounterfactualVisibility => "promote_counterfactual_visibility",
            Self::StaleFactDetected => "stale_fact_detected",
            Self::ProvenanceMissing => "provenance_missing",
            Self::DistillerQueueBacklog => "distiller_queue_backlog",
        }
    }
}

/// Closed enum of severities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAlertSeverity {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Critical.
    Critical,
}

impl MirrorAlertSeverity {
    /// Lossless string form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}
