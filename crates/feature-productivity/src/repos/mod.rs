pub mod activity_category;
pub mod activity_event;
pub mod daily_summary;
pub mod focus_session;
pub mod nudge;

pub use activity_category::ActivityCategoryRepo;
pub use activity_event::ActivityEventRepo;
pub use daily_summary::DailySummaryRepo;
pub use focus_session::FocusSessionRepo;
pub use nudge::NudgeRepo;

/// Aggregate access to all productivity repos.
#[derive(Debug, Clone)]
pub struct ProductivityRepos {
    pub events: ActivityEventRepo,
    pub categories: ActivityCategoryRepo,
    pub sessions: FocusSessionRepo,
    pub summaries: DailySummaryRepo,
    pub nudges: NudgeRepo,
}

impl ProductivityRepos {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            events: ActivityEventRepo::new(pool.clone()),
            categories: ActivityCategoryRepo::new(pool.clone()),
            sessions: FocusSessionRepo::new(pool.clone()),
            summaries: DailySummaryRepo::new(pool.clone()),
            nudges: NudgeRepo::new(pool),
        }
    }
}
