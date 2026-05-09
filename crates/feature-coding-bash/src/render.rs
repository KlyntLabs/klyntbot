//! Rendered prose for `<system-reminder>` blocks.

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
    let tail_start = final_summary.floor_char_boundary(final_summary.len().saturating_sub(8000));
    s.push_str("\nLast portion of output:\n");
    s.push_str(&final_summary[tail_start..]);
    s.push_str("\n</system-reminder>");
    s
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
