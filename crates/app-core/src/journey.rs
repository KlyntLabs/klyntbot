//! JourneyTracker — persists onboarding milestone completions as a u32 bitfield
//! in the `user_preferences` table (key = `'journey_milestones'`).

use storage::StoragePool;

const PREF_KEY: &str = "journey_milestones";

/// Onboarding milestones, each mapped to a single bit in a u32 bitfield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Milestone {
    SetupComplete = 1 << 0,
    FirstImport = 1 << 1,
    FirstChatResponse = 1 << 2,
    OrbAwakening = 1 << 3,
    FirstFocusDebrief = 1 << 4,
    FirstDotAccepted = 1 << 5,
    QuietDay = 1 << 6,
    FirstBrainReport = 1 << 7,
    HelloPulse = 1 << 8,
}

/// All milestone variants in declaration order.
const ALL_MILESTONES: &[Milestone] = &[
    Milestone::SetupComplete,
    Milestone::FirstImport,
    Milestone::FirstChatResponse,
    Milestone::OrbAwakening,
    Milestone::FirstFocusDebrief,
    Milestone::FirstDotAccepted,
    Milestone::QuietDay,
    Milestone::FirstBrainReport,
    Milestone::HelloPulse,
];

impl Milestone {
    /// Snake_case name used for frontend serialization.
    pub fn name(self) -> &'static str {
        match self {
            Self::SetupComplete => "setup_complete",
            Self::FirstImport => "first_import",
            Self::FirstChatResponse => "first_chat_response",
            Self::OrbAwakening => "orb_awakening",
            Self::FirstFocusDebrief => "first_focus_debrief",
            Self::FirstDotAccepted => "first_dot_accepted",
            Self::QuietDay => "quiet_day",
            Self::FirstBrainReport => "first_brain_report",
            Self::HelloPulse => "hello_pulse",
        }
    }

    /// Parse from the snake_case name. Returns `None` for unknown strings.
    pub fn from_name(s: &str) -> Option<Self> {
        ALL_MILESTONES.iter().find(|m| m.name() == s).copied()
    }
}

/// Tracks which onboarding milestones have been completed.
///
/// State is persisted as a u32 bitfield in the `user_preferences` table.
/// `StoragePool` is `Clone + Send + Sync`, so no `Arc` wrapping is needed.
#[derive(Debug, Clone)]
pub struct JourneyTracker {
    pool: StoragePool,
}

impl JourneyTracker {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Returns `true` if the given milestone bit is set.
    pub async fn is_complete(&self, milestone: Milestone) -> bool {
        let bits = self.load_bits().await;
        bits & (milestone as u32) != 0
    }

    /// Returns the snake_case names of all completed milestones.
    pub async fn completed_names(&self) -> Vec<String> {
        let bits = self.load_bits().await;
        ALL_MILESTONES
            .iter()
            .filter(|m| bits & (**m as u32) != 0)
            .map(|m| m.name().to_string())
            .collect()
    }

    /// ORs the milestone bit into the stored bitfield.
    pub async fn mark_complete(&self, milestone: Milestone) {
        let bits = self.load_bits().await;
        self.save_bits(bits | (milestone as u32)).await;
    }

    /// Counts total items across tasks + notes + finance transactions.
    ///
    /// Used as a guard for the `HelloPulse` milestone (requires >= 3 items).
    pub async fn total_item_count(&self) -> i64 {
        let db = self.pool.inner();
        let (tasks, notes, finance) = tokio::join!(
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM tasks").fetch_one(db),
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM notes").fetch_one(db),
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM finance_transactions").fetch_one(db),
        );
        tasks.map_or(0, |x| x.0) + notes.map_or(0, |x| x.0) + finance.map_or(0, |x| x.0)
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    async fn load_bits(&self) -> u32 {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM user_preferences WHERE key = ?1")
                .bind(PREF_KEY)
                .fetch_optional(self.pool.inner())
                .await
                .unwrap_or(None);

        row.and_then(|(v,)| v.parse::<u32>().ok()).unwrap_or(0)
    }

    async fn save_bits(&self, bits: u32) {
        let now = jiff::Timestamp::now().to_string();
        let _ = sqlx::query(
            "INSERT INTO user_preferences (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(PREF_KEY)
        .bind(bits.to_string())
        .bind(&now)
        .execute(self.pool.inner())
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_tracker() -> JourneyTracker {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        JourneyTracker::new(pool)
    }

    #[tokio::test]
    async fn test_milestone_roundtrip() {
        let tracker = make_tracker().await;

        // Initially not complete.
        assert!(!tracker.is_complete(Milestone::SetupComplete).await);

        tracker.mark_complete(Milestone::SetupComplete).await;

        // Now complete.
        assert!(tracker.is_complete(Milestone::SetupComplete).await);

        // Other milestones are still false.
        assert!(!tracker.is_complete(Milestone::FirstImport).await);
        assert!(!tracker.is_complete(Milestone::FirstChatResponse).await);
        assert!(!tracker.is_complete(Milestone::OrbAwakening).await);
    }

    #[tokio::test]
    async fn test_multiple_milestones() {
        let tracker = make_tracker().await;

        tracker.mark_complete(Milestone::FirstImport).await;
        tracker.mark_complete(Milestone::OrbAwakening).await;

        assert!(tracker.is_complete(Milestone::FirstImport).await);
        assert!(tracker.is_complete(Milestone::OrbAwakening).await);

        // Third milestone not marked — must be false.
        assert!(!tracker.is_complete(Milestone::FirstFocusDebrief).await);
    }
}
