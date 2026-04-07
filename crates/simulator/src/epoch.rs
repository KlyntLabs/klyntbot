use chrono::{DateTime, Datelike, Duration, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// EpochStep — discrete time increments
// ---------------------------------------------------------------------------

/// A discrete time step for simulation advancement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EpochStep {
    /// Advance by a specific number of minutes.
    Minutes(u32),
    /// Advance by a specific number of hours.
    Hours(u32),
    /// Advance by exactly one day (24 hours).
    Day,
    /// Advance by exactly one week (7 days / 168 hours).
    Week,
}

impl EpochStep {
    /// Convert the step into a `chrono::Duration`.
    pub fn to_duration(&self) -> Duration {
        match self {
            EpochStep::Minutes(m) => Duration::minutes(i64::from(*m)),
            EpochStep::Hours(h) => Duration::hours(i64::from(*h)),
            EpochStep::Day => Duration::hours(24),
            EpochStep::Week => Duration::hours(168),
        }
    }

    /// Whether this step is sub-day (multiple epochs per day).
    pub fn is_sub_day(&self) -> bool {
        match self {
            EpochStep::Minutes(_) => true,
            EpochStep::Hours(h) => *h < 24,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// CronTrigger — simulated cron jobs
// ---------------------------------------------------------------------------

/// Identifies a cron job that should fire during a simulation tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CronTrigger {
    /// Atom salience decay — daily at 03:00 UTC.
    AtomDecay,
    /// Autotuner nightly evaluation — daily at 02:00 UTC.
    AutotunerNightly,
    /// Analytics / log cleanup — daily on midnight crossing.
    AnalyticsCleanup,
    /// Memory maintenance — every 12 h at 00:00 and 12:00 UTC.
    MemoryMaintenance,
    /// Cognitive reflection — Monday at 09:00 UTC.
    CognitiveReflection,
    /// Mirror weekly narrative — Sunday at 10:00 UTC.
    MirrorWeeklyNarrative,
    /// Mirror cleanup — Sunday at 04:00 UTC.
    MirrorCleanup,
    /// Cross-domain insight — daily at 02:00 UTC.
    CrossDomainInsight,
}

// ---------------------------------------------------------------------------
// EpochPlan — the result of advancing one tick
// ---------------------------------------------------------------------------

/// Describes what happened during a single simulation tick.
#[derive(Debug)]
pub struct EpochPlan {
    /// The simulated time *after* this tick.
    pub simulated_now: DateTime<Utc>,
    /// The simulated time *before* this tick (i.e. the previous `simulated_now`).
    pub previous: DateTime<Utc>,
    /// Cron jobs that should fire *before* user messages in this tick.
    pub cron_pre_message: Vec<CronTrigger>,
    /// Cron jobs that should fire *after* user messages in this tick.
    pub cron_post_message: Vec<CronTrigger>,
    /// 1-based day counter within the simulation.
    pub day_of_simulation: u32,
}

// ---------------------------------------------------------------------------
// SimulatedEpoch — the time controller
// ---------------------------------------------------------------------------

/// Discrete time controller that advances simulated time in fixed steps and
/// determines which cron jobs should fire during each tick.
#[derive(Debug)]
pub struct SimulatedEpoch {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    step: EpochStep,
    current: DateTime<Utc>,
}

impl SimulatedEpoch {
    /// Create a new epoch controller.
    ///
    /// `start` is the initial simulated time, `end` is the terminal boundary
    /// (exclusive), and `step` is the fixed increment applied on each
    /// [`advance`](Self::advance) call.
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>, step: EpochStep) -> Self {
        Self {
            start,
            end,
            step,
            current: start,
        }
    }

    /// The current simulated time.
    pub fn current(&self) -> DateTime<Utc> {
        self.current
    }

    /// Returns `true` when the simulation has reached or passed the end time.
    pub fn is_finished(&self) -> bool {
        self.current >= self.end
    }

    /// 1-based day counter since the simulation started.
    pub fn day_of_simulation(&self) -> u32 {
        let elapsed = self.current.signed_duration_since(self.start);
        let days = elapsed.num_seconds() / 86_400;
        (days as u32) + 1
    }

    /// Advance the clock by one step, returning an [`EpochPlan`] describing
    /// the tick. Returns `None` if the simulation is already finished.
    pub fn advance(&mut self) -> Option<EpochPlan> {
        if self.is_finished() {
            return None;
        }

        let prev = self.current;
        let mut next = prev + self.step.to_duration();

        // Clamp to end so we never overshoot.
        if next > self.end {
            next = self.end;
        }

        self.current = next;

        let cron_pre_message = collect_pre_message_crons(prev, next);
        let cron_post_message = collect_post_message_crons(prev, next);

        Some(EpochPlan {
            simulated_now: next,
            previous: prev,
            cron_pre_message,
            cron_post_message,
            day_of_simulation: self.day_of_simulation(),
        })
    }
}

// ---------------------------------------------------------------------------
// Cron scheduling helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the half-open interval `(prev, now]` contains a midnight
/// crossing (i.e. the UTC clock reads 00:00:00 at some point strictly after
/// `prev` and at or before `now`).
pub fn crosses_midnight(prev: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    crosses_daily_hour(prev, now, 0)
}

/// Returns `true` if the half-open interval `(prev, now]` contains a point at
/// which the UTC clock reads `target_hour:00:00` on *any* day.
pub fn crosses_daily_hour(prev: DateTime<Utc>, now: DateTime<Utc>, target_hour: u32) -> bool {
    if now <= prev {
        return false;
    }

    let target_time = NaiveTime::from_hms_opt(target_hour, 0, 0).unwrap();
    let first_day = prev.date_naive();
    let last_day = now.date_naive();

    let mut day = first_day;
    loop {
        let candidate = day.and_time(target_time).and_utc();
        // Half-open: (prev, now]
        if candidate > prev && candidate <= now {
            return true;
        }
        if day >= last_day {
            break;
        }
        day = day.succ_opt().unwrap();
    }

    false
}

/// Returns `true` if the half-open interval `(prev, now]` contains a point at
/// which the UTC clock reads `target_hour:00:00` on a specific `weekday`.
pub fn crosses_weekday_hour(
    prev: DateTime<Utc>,
    now: DateTime<Utc>,
    weekday: Weekday,
    target_hour: u32,
) -> bool {
    if now <= prev {
        return false;
    }

    let target_time = NaiveTime::from_hms_opt(target_hour, 0, 0).unwrap();
    let first_day = prev.date_naive();
    let last_day = now.date_naive();

    let mut day = first_day;
    loop {
        if day.weekday() == weekday {
            let candidate = day.and_time(target_time).and_utc();
            if candidate > prev && candidate <= now {
                return true;
            }
        }
        if day >= last_day {
            break;
        }
        day = day.succ_opt().unwrap();
    }

    false
}

// ---------------------------------------------------------------------------
// Pre-message crons (fire before user messages in a tick)
// ---------------------------------------------------------------------------

fn collect_pre_message_crons(prev: DateTime<Utc>, now: DateTime<Utc>) -> Vec<CronTrigger> {
    let mut triggers = Vec::new();

    // AtomDecay — daily at 03:00 UTC
    if crosses_daily_hour(prev, now, 3) {
        triggers.push(CronTrigger::AtomDecay);
    }

    // AutotunerNightly — daily at 02:00 UTC
    if crosses_daily_hour(prev, now, 2) {
        triggers.push(CronTrigger::AutotunerNightly);
    }

    // AnalyticsCleanup — daily on midnight crossing
    if crosses_midnight(prev, now) {
        triggers.push(CronTrigger::AnalyticsCleanup);
    }

    // MemoryMaintenance — every 12h at 00:00 and 12:00 UTC
    if crosses_daily_hour(prev, now, 0) || crosses_daily_hour(prev, now, 12) {
        triggers.push(CronTrigger::MemoryMaintenance);
    }

    triggers
}

// ---------------------------------------------------------------------------
// Post-message crons (fire after user messages in a tick)
// ---------------------------------------------------------------------------

fn collect_post_message_crons(prev: DateTime<Utc>, now: DateTime<Utc>) -> Vec<CronTrigger> {
    let mut triggers = Vec::new();

    // CognitiveReflection — Monday at 09:00 UTC
    if crosses_weekday_hour(prev, now, Weekday::Mon, 9) {
        triggers.push(CronTrigger::CognitiveReflection);
    }

    // MirrorWeeklyNarrative — Sunday at 10:00 UTC
    if crosses_weekday_hour(prev, now, Weekday::Sun, 10) {
        triggers.push(CronTrigger::MirrorWeeklyNarrative);
    }

    // MirrorCleanup — Sunday at 04:00 UTC
    if crosses_weekday_hour(prev, now, Weekday::Sun, 4) {
        triggers.push(CronTrigger::MirrorCleanup);
    }

    // CrossDomainInsight — daily at 02:00 UTC
    if crosses_daily_hour(prev, now, 2) {
        triggers.push(CronTrigger::CrossDomainInsight);
    }

    triggers
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashSet;

    /// Helper: build a UTC datetime.
    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn epoch_advances_by_day() {
        let start = utc(2026, 3, 1, 8, 0);
        let end = utc(2026, 3, 4, 8, 0); // 3 days
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        assert_eq!(epoch.day_of_simulation(), 1);
        assert_eq!(epoch.current(), start);

        let plan = epoch.advance().unwrap();
        assert_eq!(plan.simulated_now, utc(2026, 3, 2, 8, 0));
        assert_eq!(plan.previous, start);
        assert_eq!(epoch.day_of_simulation(), 2);
        assert_eq!(epoch.current(), utc(2026, 3, 2, 8, 0));

        let plan2 = epoch.advance().unwrap();
        assert_eq!(plan2.simulated_now, utc(2026, 3, 3, 8, 0));
        assert_eq!(epoch.day_of_simulation(), 3);

        let plan3 = epoch.advance().unwrap();
        assert_eq!(plan3.simulated_now, utc(2026, 3, 4, 8, 0));
        assert!(epoch.is_finished());
    }

    #[test]
    fn epoch_finishes_at_end() {
        let start = utc(2026, 3, 1, 0, 0);
        let end = utc(2026, 3, 2, 0, 0); // exactly 1 day
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        assert!(!epoch.is_finished());
        let plan = epoch.advance().unwrap();
        assert_eq!(plan.simulated_now, end);
        assert!(epoch.is_finished());

        // Subsequent advance returns None.
        assert!(epoch.advance().is_none());
    }

    #[test]
    fn daily_crons_fire_each_day() {
        // Start at 2026-03-01 23:00, step by Day (24h) -> ends at 2026-03-02 23:00.
        // The interval (23:00 Mar 1, 23:00 Mar 2] crosses midnight, 02:00, 03:00, 12:00.
        let start = utc(2026, 3, 1, 23, 0);
        let end = utc(2026, 3, 3, 23, 0);
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        let plan = epoch.advance().unwrap();

        let pre: HashSet<CronTrigger> = plan.cron_pre_message.iter().copied().collect();
        assert!(
            pre.contains(&CronTrigger::AtomDecay),
            "AtomDecay should fire (03:00 crossed)"
        );
        assert!(
            pre.contains(&CronTrigger::AutotunerNightly),
            "AutotunerNightly should fire (02:00 crossed)"
        );
        assert!(
            pre.contains(&CronTrigger::AnalyticsCleanup),
            "AnalyticsCleanup should fire (midnight crossed)"
        );
        assert!(
            pre.contains(&CronTrigger::MemoryMaintenance),
            "MemoryMaintenance should fire (00:00 and/or 12:00 crossed)"
        );

        // Post-message: CrossDomainInsight fires daily at 02:00.
        let post: HashSet<CronTrigger> = plan.cron_post_message.iter().copied().collect();
        assert!(
            post.contains(&CronTrigger::CrossDomainInsight),
            "CrossDomainInsight should fire (02:00 crossed)"
        );
    }

    #[test]
    fn monday_reflection_fires_on_monday() {
        // 2026-03-01 is a Sunday.
        let sunday_8am = utc(2026, 3, 1, 8, 0);
        assert_eq!(sunday_8am.weekday(), Weekday::Sun);

        let end = utc(2026, 3, 5, 8, 0); // Thursday
        let mut epoch = SimulatedEpoch::new(sunday_8am, end, EpochStep::Day);

        // Sunday 08:00 -> Monday 08:00: crosses Monday 00:00..08:00 but NOT 09:00.
        // Wait — Mon 09:00 is NOT in (Sun 08:00, Mon 08:00]. Need to step again.
        let plan_sun_to_mon = epoch.advance().unwrap();
        let post: HashSet<CronTrigger> =
            plan_sun_to_mon.cron_post_message.iter().copied().collect();
        // Mon 09:00 is at hour 9, but our window ends at Mon 08:00 — does NOT fire.
        assert!(
            !post.contains(&CronTrigger::CognitiveReflection),
            "CognitiveReflection should NOT fire Sun 08:00 -> Mon 08:00 (Mon 09:00 not in range)"
        );

        // Monday 08:00 -> Tuesday 08:00: crosses Monday 09:00 — SHOULD fire.
        let plan_mon_to_tue = epoch.advance().unwrap();
        let post2: HashSet<CronTrigger> =
            plan_mon_to_tue.cron_post_message.iter().copied().collect();
        assert!(
            post2.contains(&CronTrigger::CognitiveReflection),
            "CognitiveReflection should fire Mon 08:00 -> Tue 08:00 (Mon 09:00 in range)"
        );

        // Tuesday 08:00 -> Wednesday 08:00: should NOT fire.
        let plan_tue_to_wed = epoch.advance().unwrap();
        let post3: HashSet<CronTrigger> =
            plan_tue_to_wed.cron_post_message.iter().copied().collect();
        assert!(
            !post3.contains(&CronTrigger::CognitiveReflection),
            "CognitiveReflection should NOT fire Tue -> Wed"
        );
    }

    #[test]
    fn weekly_step_hits_all_crons() {
        // 2026-03-04 is a Wednesday.
        let wed = utc(2026, 3, 4, 6, 0);
        assert_eq!(wed.weekday(), Weekday::Wed);

        let end = utc(2026, 3, 18, 6, 0);
        let mut epoch = SimulatedEpoch::new(wed, end, EpochStep::Week);

        // One week step: Wed Mar 4 06:00 -> Wed Mar 11 06:00.
        // Crosses: Thu, Fri, Sat, Sun (04:00 & 10:00), Mon (09:00), Tue, Wed.
        // Daily crons fire multiple times but we only check presence.
        let plan = epoch.advance().unwrap();

        let all: HashSet<CronTrigger> = plan
            .cron_pre_message
            .iter()
            .chain(plan.cron_post_message.iter())
            .copied()
            .collect();

        assert!(all.contains(&CronTrigger::AtomDecay));
        assert!(all.contains(&CronTrigger::AutotunerNightly));
        assert!(all.contains(&CronTrigger::AnalyticsCleanup));
        assert!(all.contains(&CronTrigger::MemoryMaintenance));
        assert!(all.contains(&CronTrigger::CognitiveReflection));
        assert!(all.contains(&CronTrigger::MirrorWeeklyNarrative));
        assert!(all.contains(&CronTrigger::MirrorCleanup));
        assert!(all.contains(&CronTrigger::CrossDomainInsight));
    }

    #[test]
    fn hours_step_no_cron_if_no_crossing() {
        let start = utc(2026, 3, 1, 10, 0);
        let end = utc(2026, 3, 1, 14, 0);
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Hours(2));

        let plan = epoch.advance().unwrap();
        // 10:00 -> 12:00 crosses 12:00 (MemoryMaintenance).
        assert!(plan
            .cron_pre_message
            .contains(&CronTrigger::MemoryMaintenance));
        // But should NOT contain AtomDecay (03:00) or midnight crons.
        assert!(!plan.cron_pre_message.contains(&CronTrigger::AtomDecay));
        assert!(!plan
            .cron_pre_message
            .contains(&CronTrigger::AnalyticsCleanup));

        let plan2 = epoch.advance().unwrap();
        // 12:00 -> 14:00 — no pre-message crons.
        assert!(plan2.cron_pre_message.is_empty());
    }

    #[test]
    fn crosses_daily_hour_boundary() {
        // Exactly ON the boundary: (prev, now] means now == target is included.
        let prev = utc(2026, 3, 1, 2, 0);
        let now = utc(2026, 3, 1, 3, 0);
        assert!(crosses_daily_hour(prev, now, 3));

        // prev == target is NOT included (half-open).
        assert!(!crosses_daily_hour(prev, now, 2));
    }

    #[test]
    fn clamping_does_not_overshoot() {
        let start = utc(2026, 3, 1, 0, 0);
        let end = utc(2026, 3, 1, 10, 0);
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        // Step is 24h but end is only 10h away — should clamp.
        let plan = epoch.advance().unwrap();
        assert_eq!(plan.simulated_now, end);
        assert!(epoch.is_finished());
    }

    #[test]
    fn day_of_simulation_counter() {
        let start = utc(2026, 3, 1, 0, 0);
        let end = utc(2026, 3, 8, 0, 0); // 7 days
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        assert_eq!(epoch.day_of_simulation(), 1);
        epoch.advance();
        assert_eq!(epoch.day_of_simulation(), 2);
        epoch.advance();
        assert_eq!(epoch.day_of_simulation(), 3);
    }

    #[test]
    fn minutes_step_fires_daily_cron_once() {
        // 48 steps of 30min = 24 hours. AtomDecay at 03:00 should fire exactly once.
        let start = utc(2026, 4, 1, 0, 0);
        let end = start + Duration::hours(24);
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Minutes(30));

        let mut atom_decay_count = 0;
        while let Some(plan) = epoch.advance() {
            for cron in &plan.cron_pre_message {
                if matches!(cron, CronTrigger::AtomDecay) {
                    atom_decay_count += 1;
                }
            }
        }
        assert_eq!(
            atom_decay_count, 1,
            "AtomDecay should fire exactly once in 24h"
        );
    }

    #[test]
    fn sunday_crons_fire_on_sunday() {
        // Start Saturday 20:00, end Monday 00:00 — Sunday is fully covered.
        let start = utc(2026, 3, 7, 20, 0); // Saturday
        assert_eq!(start.weekday(), Weekday::Sat);
        let end = utc(2026, 3, 9, 20, 0);
        let mut epoch = SimulatedEpoch::new(start, end, EpochStep::Day);

        // Sat 20:00 -> Sun 20:00
        let plan = epoch.advance().unwrap();
        let post: HashSet<CronTrigger> = plan.cron_post_message.iter().copied().collect();
        assert!(post.contains(&CronTrigger::MirrorWeeklyNarrative));
        assert!(post.contains(&CronTrigger::MirrorCleanup));
    }
}
