//! Rendered prose for `<system-reminder>` blocks.

use crate::intelligence::{ExtractedDiff, JobDiff, KindTransition};
use jiff::Timestamp;
use tools_core::{GateResult, JobId, JobSpec, JobView};

pub fn active_jobs_reminder(jobs: &[JobView]) -> String {
    let now = Timestamp::now();
    let mut s = String::from("<system-reminder>\n");
    s.push_str(&format!(
        "You have {} background job(s) running in this thread:\n",
        jobs.len()
    ));
    for j in jobs {
        let elapsed = (now - j.started_at)
            .total(jiff::Unit::Second)
            .unwrap_or(0.0) as u64;
        let bytes_h = human_bytes(j.total_bytes_emitted);
        s.push_str(&format!(
            "- {}: {} (started {}s ago, {} output)\n",
            j.id.as_str(),
            j.description,
            elapsed,
            bytes_h,
        ));
    }
    s.push_str("\nInspect output with coding_task_output(task_id, since_offset).\n");
    s.push_str("Cancel with coding_task_stop(task_id).\n");
    s.push_str("Completed jobs auto-notify in this thread.\n");
    s.push_str("</system-reminder>");
    s
}

pub fn completion_notification(
    id: &JobId,
    spec: &JobSpec,
    result: &GateResult,
    final_summary: &str,
    diff: Option<&JobDiff>,
) -> String {
    let mut s = String::from("<system-reminder>\n");
    s.push_str(&format!("Background job {} completed.\n", id.as_str()));
    s.push_str(&format!("Description: {}\n", spec.description));
    match result {
        GateResult::Passed => s.push_str("Status: Completed (Passed)\n"),
        GateResult::Failed {
            kind,
            detail,
            extracted,
        } => {
            s.push_str(&format!(
                "Status: Failed\nFailure kind: {kind:?}\nDetail: {detail}\n"
            ));
            if !extracted.is_null() {
                s.push_str(&format!(
                    "Extracted: {}\n",
                    serde_json::to_string_pretty(extracted).unwrap_or_default()
                ));
            }
        }
    }

    if let Some(d) = diff {
        s.push_str("\nCompared to last run of this command:\n");
        s.push_str(&format_kind_transition(&d.kind_transition));
        s.push_str(&format_extracted_diff(&d.extracted_diff));
        s.push_str(&format!("  Wall-clock: {}\n", format_elapsed_delta(d.elapsed_delta_ms)));
    }

    let tail_start = final_summary.floor_char_boundary(final_summary.len().saturating_sub(8000));
    s.push_str("\nLast portion of output:\n");
    s.push_str(&final_summary[tail_start..]);
    s.push_str("\n</system-reminder>");
    s
}

fn format_kind_transition(t: &KindTransition) -> String {
    match t {
        KindTransition::StillPassing => "  Transition: StillPassing\n".into(),
        KindTransition::StillFailing { kind } => format!("  Transition: StillFailing ({kind})\n"),
        KindTransition::Regressed { from, to } => format!("  Transition: Regressed ({from} → {to})\n"),
        KindTransition::Recovered { prior_kind } => format!("  Transition: Recovered ({prior_kind} → Passed)\n"),
        KindTransition::Changed { from, to } => format!("  Transition: Changed ({from} → {to})\n"),
    }
}

fn format_extracted_diff(d: &ExtractedDiff) -> String {
    match d {
        ExtractedDiff::None => String::new(),
        ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
            let mut s = String::from("  Test diff:\n");
            s.push_str(&format!("    new failures:  {}\n", trim_set(new_failures)));
            s.push_str(&format!("    still failing: {}\n", trim_set(still_failing)));
            s.push_str(&format!("    resolved:      {}\n", trim_set(resolved)));
            s
        }
        ExtractedDiff::Compile { same_location, prior_loc, curr_loc } => {
            let same = if *same_location { "same location" } else { "different location" };
            format!(
                "  Compile diff: {same} (prior: {prior_loc:?}, curr: {curr_loc:?})\n"
            )
        }
        ExtractedDiff::Bind { same_port, prior_port, curr_port } => {
            let same = if *same_port { "same port" } else { "different port" };
            format!("  Bind diff: {same} (prior: {prior_port:?}, curr: {curr_port:?})\n")
        }
        ExtractedDiff::Lint { delta_n_errors } => {
            format!("  Lint diff: error count Δ {delta_n_errors:+}\n")
        }
        ExtractedDiff::Timeout { prior_ms, curr_ms } => {
            format!("  Timeout diff: prior {prior_ms} ms, curr {curr_ms} ms\n")
        }
        ExtractedDiff::OtherExitTransition { from, to } => {
            format!("  Exit code transition: {from:?} → {to:?}\n")
        }
    }
}

fn trim_set(v: &[String]) -> String {
    if v.is_empty() {
        return "(none)".into();
    }
    let mut sorted: Vec<&str> = v.iter().map(|s| s.as_str()).collect();
    sorted.sort();
    if sorted.len() <= 50 {
        sorted.join(", ")
    } else {
        let head = sorted[..50].join(", ");
        format!("{head}, + {} more", sorted.len() - 50)
    }
}

fn format_elapsed_delta(ms: i64) -> String {
    let secs = ms.abs() as f64 / 1000.0;
    if ms > 0 {
        format!("+{secs:.1}s (slower)")
    } else if ms < 0 {
        format!("-{secs:.1}s (faster)")
    } else {
        "+0.0s".into()
    }
}

/// Format byte count into human-readable string (e.g. "1.5 MB").
pub fn human_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}
