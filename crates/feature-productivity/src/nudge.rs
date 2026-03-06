//! NudgeService — background loop that sends break reminders and burnout alerts.

use chrono::{DateTime, Duration, Local, NaiveTime, Utc};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::config::{FocusConfig, NudgeConfig};
use crate::distraction_analyzer::DistractionAnalyzer;
use crate::repos::ProductivityRepos;
use crate::types::{NudgeRecord, NudgeType};

/// Burnout alerts use a longer cooldown than regular nudges (4× the base cooldown).
const BURNOUT_COOLDOWN_MULTIPLIER: u64 = 4;

pub struct NudgeService {
    repos: ProductivityRepos,
    nudge_config: NudgeConfig,
    focus_config: FocusConfig,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
    nudge_sender: mpsc::Sender<NudgeRecord>,
}

impl NudgeService {
    pub fn new(
        repos: ProductivityRepos,
        nudge_config: NudgeConfig,
        focus_config: FocusConfig,
        nudge_sender: mpsc::Sender<NudgeRecord>,
    ) -> Self {
        Self {
            repos,
            nudge_config,
            focus_config,
            cancel_token: CancellationToken::new(),
            task_handle: None,
            nudge_sender,
        }
    }

    pub fn start(&mut self) {
        if self.task_handle.is_some() {
            warn!("NudgeService already running — ignoring duplicate start");
            return;
        }
        let cancel = self.cancel_token.clone();
        let repos = self.repos.clone();
        let nudge_config = self.nudge_config.clone();
        let focus_config = self.focus_config.clone();
        let sender = self.nudge_sender.clone();

        let handle = tokio::spawn(async move {
            let check_interval = std::time::Duration::from_secs(60);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("NudgeService shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(check_interval) => {
                        if let Err(e) = check_nudges(&repos, &nudge_config, &focus_config, &sender).await {
                            warn!("NudgeService check failed: {e}");
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
        info!("NudgeService started (60s check interval)");
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("NudgeService task panicked: {e}");
            }
        }
        info!("NudgeService stopped");
    }
}

/// Run all nudge checks for this tick.
async fn check_nudges(
    repos: &ProductivityRepos,
    config: &NudgeConfig,
    focus_config: &FocusConfig,
    sender: &mpsc::Sender<NudgeRecord>,
) -> common::Result<()> {
    // Skip during quiet hours
    if is_quiet_hours(config) {
        debug!("NudgeService: quiet hours — skipping");
        return Ok(());
    }

    let now = Utc::now();

    // 1. Break reminder — continuous active time exceeds threshold.
    //    Check cooldown first (cheap indexed lookup) to avoid the expensive SUM query.
    if config.break_reminders
        && should_send(repos, NudgeType::BreakReminder, config.cooldown_mins, now).await?
    {
        let break_threshold_secs = focus_config.break_interval_mins as i64 * 60;
        let window_start = now - Duration::seconds(break_threshold_secs);
        let active_secs = repos.events.total_active_secs(&window_start, &now).await?;

        if active_secs >= break_threshold_secs {
            let record = NudgeRecord::new(
                NudgeType::BreakReminder,
                format!(
                    "You've been active for {}+ minutes. Time for a {}-minute break!",
                    focus_config.break_interval_mins, focus_config.break_duration_mins
                ),
                now,
            );
            send_nudge(repos, sender, record).await?;
        }
    }

    // 2. Burnout alert — daily active hours exceed max.
    //    Check cooldown first to skip the daily aggregation when unnecessary.
    let burnout_cooldown = config.cooldown_mins * BURNOUT_COOLDOWN_MULTIPLIER;
    if config.burnout_alerts
        && should_send(repos, NudgeType::BurnoutAlert, burnout_cooldown, now).await?
    {
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
            .and_utc();
        let daily_active = repos.events.total_active_secs(&today_start, &now).await?;
        let max_daily_secs = focus_config.max_daily_focus_hours as i64 * 3600;

        if daily_active >= max_daily_secs {
            let hours = daily_active as f64 / 3600.0;
            let record = NudgeRecord::new(
                NudgeType::BurnoutAlert,
                format!(
                    "You've been active for {:.1}h today (max: {}h). Consider wrapping up.",
                    hours, focus_config.max_daily_focus_hours
                ),
                now,
            );
            send_nudge(repos, sender, record).await?;
        }
    }

    // 3. Proactive break suggestion — high distraction risk window.
    if config.focus_suggestions
        && should_send(repos, NudgeType::FocusSuggestion, config.cooldown_mins, now).await?
        && DistractionAnalyzer::is_high_risk_window(repos, now).await?
    {
        let record = NudgeRecord::new(
            NudgeType::FocusSuggestion,
            "You're in a historically high-distraction period. Consider a short break.".into(),
            now,
        );
        send_nudge(repos, sender, record).await?;
    }

    Ok(())
}

/// Check if enough time has passed since the last nudge of this type.
async fn should_send(
    repos: &ProductivityRepos,
    nudge_type: NudgeType,
    cooldown_mins: u64,
    now: DateTime<Utc>,
) -> common::Result<bool> {
    let last = repos.nudges.last_of_type(nudge_type).await?;
    match last {
        Some(record) => {
            let elapsed = now - record.created_at;
            Ok(elapsed.num_minutes() >= cooldown_mins as i64)
        }
        None => Ok(true),
    }
}

/// Persist nudge to DB and send via channel.
async fn send_nudge(
    repos: &ProductivityRepos,
    sender: &mpsc::Sender<NudgeRecord>,
    record: NudgeRecord,
) -> common::Result<()> {
    repos.nudges.insert(&record).await?;
    if sender.send(record).await.is_err() {
        warn!("NudgeService: receiver dropped, nudge not delivered");
    }
    Ok(())
}

/// Check if current time falls within configured quiet hours.
fn is_quiet_hours(config: &NudgeConfig) -> bool {
    let (Some(start_str), Some(end_str)) = (&config.quiet_hours_start, &config.quiet_hours_end)
    else {
        return false;
    };

    let Ok(start) = NaiveTime::parse_from_str(start_str, "%H:%M") else {
        warn!("NudgeService: invalid quiet_hours_start format: {start_str:?} (expected HH:MM)");
        return false;
    };
    let Ok(end) = NaiveTime::parse_from_str(end_str, "%H:%M") else {
        warn!("NudgeService: invalid quiet_hours_end format: {end_str:?} (expected HH:MM)");
        return false;
    };

    let now = Local::now().time();

    if start <= end {
        // Same-day range (e.g., 09:00 – 17:00)
        now >= start && now < end
    } else {
        // Overnight range (e.g., 22:00 – 07:00)
        now >= start || now < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;
    use chrono::Duration;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn default_nudge_config() -> NudgeConfig {
        NudgeConfig {
            break_reminders: true,
            focus_suggestions: true,
            daily_summary: true,
            burnout_alerts: true,
            cooldown_mins: 15,
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }

    fn default_focus_config() -> FocusConfig {
        FocusConfig::default()
    }

    #[tokio::test]
    async fn break_reminder_after_threshold() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let (tx, mut rx) = mpsc::channel(10);

        // Insert continuous active events exceeding break_interval (90 min).
        // Use 85-min window to avoid boundary race with check_nudges' own Utc::now().
        let now = Utc::now();
        for i in 0..10 {
            let event = crate::types::ActivityEvent {
                id: None,
                app_name: "Code".into(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: Some("coding".into()),
                started_at: now - Duration::minutes(85) + Duration::minutes(i * 8),
                ended_at: Some(now - Duration::minutes(85) + Duration::minutes(i * 8 + 8)),
                duration_secs: Some(540), // 9 min each = 90 min total
                is_idle: false,
                metadata: None,
                project_id: None,
            };
            repos.events.insert(&event).await.unwrap();
        }

        let config = default_nudge_config();
        let focus = default_focus_config();
        check_nudges(&repos, &config, &focus, &tx).await.unwrap();

        let nudge = rx.try_recv().unwrap();
        assert_eq!(nudge.nudge_type, NudgeType::BreakReminder);
        assert!(nudge.message.contains("90"));
    }

    #[tokio::test]
    async fn cooldown_prevents_spam() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let (tx, mut rx) = mpsc::channel(10);

        // Insert a recent nudge
        let recent = NudgeRecord::new(
            NudgeType::BreakReminder,
            "Take a break".into(),
            Utc::now() - Duration::minutes(5), // 5 min ago (cooldown is 15)
        );
        repos.nudges.insert(&recent).await.unwrap();

        // Insert active events exceeding threshold
        let now = Utc::now();
        for i in 0..10 {
            let event = crate::types::ActivityEvent {
                id: None,
                app_name: "Code".into(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: None,
                started_at: now - Duration::minutes(90) + Duration::minutes(i * 9),
                ended_at: Some(now - Duration::minutes(90) + Duration::minutes(i * 9 + 9)),
                duration_secs: Some(540),
                is_idle: false,
                metadata: None,
                project_id: None,
            };
            repos.events.insert(&event).await.unwrap();
        }

        let config = default_nudge_config();
        let focus = default_focus_config();
        check_nudges(&repos, &config, &focus, &tx).await.unwrap();

        // Should NOT receive a nudge — cooldown not elapsed
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn quiet_hours_suppresses_nudges() {
        // Test the quiet hours logic directly — use local time since is_quiet_hours uses Local::now()
        let now_time = chrono::Local::now().time();
        let start = (now_time - chrono::Duration::minutes(30))
            .format("%H:%M")
            .to_string();
        let end = (now_time + chrono::Duration::minutes(30))
            .format("%H:%M")
            .to_string();

        let config = NudgeConfig {
            quiet_hours_start: Some(start),
            quiet_hours_end: Some(end),
            ..default_nudge_config()
        };

        assert!(is_quiet_hours(&config));
    }

    #[tokio::test]
    async fn burnout_detection() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let (tx, mut rx) = mpsc::channel(10);

        // Insert 9 events right after midnight UTC today, each with 1h duration.
        // The events' started_at must be between midnight UTC and Utc::now()
        // since check_nudges queries that range internally.
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        for i in 0..9 {
            let event = crate::types::ActivityEvent {
                id: None,
                app_name: "Code".into(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: None,
                // Pack events close together so they all fit before now
                started_at: today_start + Duration::seconds(i),
                ended_at: Some(today_start + Duration::seconds(i) + Duration::hours(1)),
                duration_secs: Some(3600), // 1 hour each = 9 hours total
                is_idle: false,
                metadata: None,
                project_id: None,
            };
            repos.events.insert(&event).await.unwrap();
        }

        let config = default_nudge_config();
        let focus = default_focus_config(); // max 8h
        check_nudges(&repos, &config, &focus, &tx).await.unwrap();

        // Collect all nudges
        let mut nudges = Vec::new();
        while let Ok(n) = rx.try_recv() {
            nudges.push(n);
        }

        // Should have a burnout alert
        assert!(nudges
            .iter()
            .any(|n| n.nudge_type == NudgeType::BurnoutAlert));
    }
}
