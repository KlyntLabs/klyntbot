pub mod activity_category;
pub mod activity_event;
pub mod bucket;
pub mod calendar_event;
pub mod categorization_cache;
pub mod daily_summary;
pub mod distraction_pattern;
pub mod focus_session;
pub mod goal;
pub mod insight;
pub mod intelligence_session;
pub mod learned_rule;
pub mod narrative;
pub mod nudge;
pub mod privacy_rule;
pub mod project;
pub mod quality_score;
pub mod rule_evolution_log;
pub mod time_entry;
pub mod tracking_rule;
pub mod voice_journal;
pub mod weekly_assessment;

pub use activity_category::ActivityCategoryRepo;
pub use activity_event::ActivityEventRepo;
pub use bucket::BucketRepo;
pub use calendar_event::CalendarEventRepo;
pub use categorization_cache::CategorizationCacheRepo;
pub use daily_summary::DailySummaryRepo;
pub use distraction_pattern::DistractionPatternRepo;
pub use focus_session::FocusSessionRepo;
pub use goal::GoalRepo;
pub use insight::InsightRepo;
pub use intelligence_session::IntelligenceSessionRepo;
pub use learned_rule::LearnedRuleRepo;
pub use narrative::NarrativeRepo;
pub use nudge::NudgeRepo;
pub use privacy_rule::PrivacyRuleRepo;
pub use project::ProjectRepo;
pub use quality_score::QualityScoreRepo;
pub use rule_evolution_log::RuleEvolutionLogRepo;
pub use time_entry::TimeEntryRepo;
pub use tracking_rule::TrackingRuleRepo;
pub use voice_journal::VoiceJournalRepo;
pub use weekly_assessment::WeeklyAssessmentRepo;

/// Aggregate access to all productivity repos.
#[derive(Debug, Clone)]
pub struct ProductivityRepos {
    pub events: ActivityEventRepo,
    pub categories: ActivityCategoryRepo,
    pub sessions: FocusSessionRepo,
    pub summaries: DailySummaryRepo,
    pub nudges: NudgeRepo,
    pub goals: GoalRepo,
    pub time_entries: TimeEntryRepo,
    pub learned_rules: LearnedRuleRepo,
    pub buckets: BucketRepo,
    pub distraction_patterns: DistractionPatternRepo,
    pub insights: InsightRepo,
    pub projects: ProjectRepo,
    pub tracking_rules: TrackingRuleRepo,
    pub intelligence_sessions: IntelligenceSessionRepo,
    pub quality_scores: QualityScoreRepo,
    pub narratives: NarrativeRepo,
    pub voice_journals: VoiceJournalRepo,
    pub categorization_cache: CategorizationCacheRepo,
    pub privacy_rules: PrivacyRuleRepo,
    pub rule_evolution_log: RuleEvolutionLogRepo,
    pub calendar_events: CalendarEventRepo,
    pub weekly_assessments: WeeklyAssessmentRepo,
}

impl ProductivityRepos {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            events: ActivityEventRepo::new(pool.clone()),
            categories: ActivityCategoryRepo::new(pool.clone()),
            sessions: FocusSessionRepo::new(pool.clone()),
            summaries: DailySummaryRepo::new(pool.clone()),
            nudges: NudgeRepo::new(pool.clone()),
            goals: GoalRepo::new(pool.clone()),
            time_entries: TimeEntryRepo::new(pool.clone()),
            learned_rules: LearnedRuleRepo::new(pool.clone()),
            buckets: BucketRepo::new(pool.clone()),
            distraction_patterns: DistractionPatternRepo::new(pool.clone()),
            insights: InsightRepo::new(pool.clone()),
            projects: ProjectRepo::new(pool.clone()),
            tracking_rules: TrackingRuleRepo::new(pool.clone()),
            intelligence_sessions: IntelligenceSessionRepo::new(pool.clone()),
            quality_scores: QualityScoreRepo::new(pool.clone()),
            narratives: NarrativeRepo::new(pool.clone()),
            voice_journals: VoiceJournalRepo::new(pool.clone()),
            categorization_cache: CategorizationCacheRepo::new(pool.clone()),
            privacy_rules: PrivacyRuleRepo::new(pool.clone()),
            rule_evolution_log: RuleEvolutionLogRepo::new(pool.clone()),
            calendar_events: CalendarEventRepo::new(pool.clone()),
            weekly_assessments: WeeklyAssessmentRepo::new(pool),
        }
    }
}
