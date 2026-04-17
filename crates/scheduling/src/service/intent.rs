//! Intent evaluation and sleep/wake job classification.

use crate::types::{CatchUpPriority, CronJob, CronSchedule, IntentTrigger};

/// Classification of a job that was missed during system sleep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissedJobClass {
    /// Job was not due during the sleep window — no action needed.
    NotMissed,
    /// Job should fire immediately on wake (cheap system jobs, `Immediate` catch-up).
    Immediate,
    /// Job should be held until the intent trigger condition is met.
    Deferred,
    /// Job was a one-shot (`At`) that expired during sleep — skip it.
    Expired,
}

/// Snapshot of user presence state used for trigger evaluation.
#[derive(Debug, Clone)]
pub struct PresenceSnapshot {
    /// Seconds the user has been idle (no input).
    pub idle_secs: u64,
    /// Whether the user is considered present (active session, not away).
    pub is_user_present: bool,
    /// Minutes of continuous active use since the last idle period.
    pub continuous_active_mins: u32,
    /// When true, smart scheduling is disabled and all triggers fire immediately.
    pub smart_scheduling_disabled: bool,
}

/// Jobs that can be executed cheaply without an LLM call and should fire immediately on wake.
const CHEAP_JOBS: &[&str] = &[
    "__klyntbot_atom_decay_daily",
    "__klyntbot_session_cleanup",
    "__klyntbot_memory_maintenance",
    "__klyntbot_analytics_cleanup",
    "__klyntbot_blackboard_cleanup",
    "__klyntbot_mirror_cleanup",
    "todo_overdue_check",
    "__klyntbot_recurring_tasks",
    "__klyntbot_reminder_check",
    "todo_focus_check",
];

/// Classify whether a job was missed during sleep and how it should be handled on wake.
///
/// - If the job's `next_run_at_ms` is not within `[sleep_start_ms, now_ms)` → `NotMissed`
/// - One-shot (`At`) jobs that were missed → `Expired`
/// - Jobs with `IntentWindow` and `WhenPresent` or `WhenIdle` catch-up → `Deferred`
/// - Jobs with `IntentWindow` and `Immediate` catch-up → `Immediate`
/// - Cheap jobs (in `CHEAP_JOBS`) → `Immediate`
/// - Everything else → `Deferred`
pub fn classify_missed_job(job: &CronJob, sleep_start_ms: i64, now_ms: i64) -> MissedJobClass {
    let next_run = match job.state.next_run_at_ms {
        Some(t) => t,
        None => return MissedJobClass::NotMissed,
    };

    // Job was not due during the sleep window
    if next_run < sleep_start_ms || next_run >= now_ms {
        return MissedJobClass::NotMissed;
    }

    // One-shot jobs that expired during sleep cannot be meaningfully retried
    if matches!(job.schedule, CronSchedule::At { .. }) {
        return MissedJobClass::Expired;
    }

    // Intent window determines catch-up behaviour
    if let Some(window) = &job.intent_window {
        return match window.catch_up {
            CatchUpPriority::Immediate => MissedJobClass::Immediate,
            CatchUpPriority::WhenPresent | CatchUpPriority::WhenIdle => MissedJobClass::Deferred,
        };
    }

    // Cheap jobs can run on wake without LLM involvement
    if CHEAP_JOBS.contains(&job.name.as_str()) {
        return MissedJobClass::Immediate;
    }

    MissedJobClass::Deferred
}

/// Evaluate whether an intent trigger condition is currently satisfied.
///
/// If `presence.smart_scheduling_disabled` is `true`, all triggers return `true`.
pub fn evaluate_trigger(trigger: &IntentTrigger, presence: &PresenceSnapshot) -> bool {
    if presence.smart_scheduling_disabled {
        return true;
    }

    match trigger {
        IntentTrigger::UserPresent => presence.is_user_present,
        IntentTrigger::MinActiveMinutes { minutes } => presence.continuous_active_mins >= *minutes,
        IntentTrigger::UserIdle { min_idle_secs } => presence.idle_secs >= *min_idle_secs,
        IntentTrigger::FirstActivityAfter { after_local } => {
            presence.is_user_present && jiff::Zoned::now().datetime().time() >= *after_local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CatchUpPriority, CronJob, CronOrigin, CronSchedule, IntentTrigger, IntentWindow,
    };

    fn make_job_with_next_run(
        name: &str,
        schedule: CronSchedule,
        next_run_at_ms: Option<i64>,
    ) -> CronJob {
        let mut job = CronJob::new("id1", name, schedule, "", CronOrigin::System);
        job.state.next_run_at_ms = next_run_at_ms;
        job
    }

    fn presence(is_present: bool) -> PresenceSnapshot {
        PresenceSnapshot {
            idle_secs: 0,
            is_user_present: is_present,
            continuous_active_mins: 0,
            smart_scheduling_disabled: false,
        }
    }

    // ── classify_missed_job ────────────────────────────────────────────────

    #[test]
    fn not_missed_if_outside_window() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;

        // next_run before sleep started
        let job = make_job_with_next_run(
            "some_job",
            CronSchedule::Every { every_ms: 60_000 },
            Some(sleep_start - 1),
        );
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::NotMissed
        );

        // next_run at or after now (not yet due)
        let job = make_job_with_next_run(
            "some_job",
            CronSchedule::Every { every_ms: 60_000 },
            Some(now),
        );
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::NotMissed
        );

        // No next_run_at_ms
        let job =
            make_job_with_next_run("some_job", CronSchedule::Every { every_ms: 60_000 }, None);
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::NotMissed
        );
    }

    #[test]
    fn cheap_job_is_immediate() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;
        let mid = 1_500_000;

        let job = make_job_with_next_run(
            "__klyntbot_session_cleanup",
            CronSchedule::Every {
                every_ms: 3_600_000,
            },
            Some(mid),
        );
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::Immediate
        );
    }

    #[test]
    fn at_job_is_expired() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;
        let mid = 1_500_000;

        let job = make_job_with_next_run("one_shot", CronSchedule::At { at_ms: mid }, Some(mid));
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::Expired
        );
    }

    #[test]
    fn llm_job_with_when_present_is_deferred() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;
        let mid = 1_500_000;

        let mut job = make_job_with_next_run(
            "weekly_review",
            CronSchedule::Every {
                every_ms: 604_800_000,
            },
            Some(mid),
        );
        job.intent_window = Some(IntentWindow {
            trigger: IntentTrigger::UserPresent,
            tolerance: std::time::Duration::from_secs(7200),
            catch_up: CatchUpPriority::WhenPresent,
        });
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::Deferred
        );
    }

    #[test]
    fn intent_window_immediate_catch_up_is_immediate() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;
        let mid = 1_500_000;

        let mut job = make_job_with_next_run(
            "sync_job",
            CronSchedule::Every { every_ms: 300_000 },
            Some(mid),
        );
        job.intent_window = Some(IntentWindow {
            trigger: IntentTrigger::UserPresent,
            tolerance: std::time::Duration::from_secs(300),
            catch_up: CatchUpPriority::Immediate,
        });
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::Immediate
        );
    }

    #[test]
    fn unknown_job_defaults_to_deferred() {
        let sleep_start = 1_000_000;
        let now = 2_000_000;
        let mid = 1_500_000;

        let job = make_job_with_next_run(
            "user_custom_job_xyz",
            CronSchedule::Every {
                every_ms: 3_600_000,
            },
            Some(mid),
        );
        assert_eq!(
            classify_missed_job(&job, sleep_start, now),
            MissedJobClass::Deferred
        );
    }

    // ── evaluate_trigger ──────────────────────────────────────────────────

    #[test]
    fn evaluate_user_present_when_present() {
        let trigger = IntentTrigger::UserPresent;
        assert!(evaluate_trigger(&trigger, &presence(true)));
    }

    #[test]
    fn evaluate_user_present_when_absent() {
        let trigger = IntentTrigger::UserPresent;
        assert!(!evaluate_trigger(&trigger, &presence(false)));
    }

    #[test]
    fn smart_scheduling_disabled_always_fires() {
        let trigger = IntentTrigger::UserPresent;
        let snap = PresenceSnapshot {
            idle_secs: 0,
            is_user_present: false,
            continuous_active_mins: 0,
            smart_scheduling_disabled: true,
        };
        assert!(evaluate_trigger(&trigger, &snap));
    }

    #[test]
    fn evaluate_min_active_minutes() {
        let trigger = IntentTrigger::MinActiveMinutes { minutes: 10 };

        let below = PresenceSnapshot {
            idle_secs: 0,
            is_user_present: true,
            continuous_active_mins: 9,
            smart_scheduling_disabled: false,
        };
        assert!(!evaluate_trigger(&trigger, &below));

        let at = PresenceSnapshot {
            continuous_active_mins: 10,
            ..below.clone()
        };
        assert!(evaluate_trigger(&trigger, &at));

        let above = PresenceSnapshot {
            continuous_active_mins: 15,
            ..below
        };
        assert!(evaluate_trigger(&trigger, &above));
    }

    #[test]
    fn evaluate_user_idle() {
        let trigger = IntentTrigger::UserIdle { min_idle_secs: 300 };

        let not_idle = PresenceSnapshot {
            idle_secs: 100,
            is_user_present: false,
            continuous_active_mins: 0,
            smart_scheduling_disabled: false,
        };
        assert!(!evaluate_trigger(&trigger, &not_idle));

        let idle = PresenceSnapshot {
            idle_secs: 300,
            ..not_idle.clone()
        };
        assert!(evaluate_trigger(&trigger, &idle));

        let very_idle = PresenceSnapshot {
            idle_secs: 600,
            ..not_idle
        };
        assert!(evaluate_trigger(&trigger, &very_idle));
    }
}
