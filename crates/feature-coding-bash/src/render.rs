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
        s.push_str(&format!(
            "  Wall-clock: {}\n",
            format_elapsed_delta(d.elapsed_delta_ms)
        ));
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
        KindTransition::Regressed { from, to } => {
            format!("  Transition: Regressed ({from} → {to})\n")
        }
        KindTransition::Recovered { prior_kind } => {
            format!("  Transition: Recovered ({prior_kind} → Passed)\n")
        }
        KindTransition::Changed { from, to } => format!("  Transition: Changed ({from} → {to})\n"),
    }
}

fn format_extracted_diff(d: &ExtractedDiff) -> String {
    match d {
        ExtractedDiff::None => String::new(),
        ExtractedDiff::TestSet {
            new_failures,
            still_failing,
            resolved,
        } => {
            let mut s = String::from("  Test diff:\n");
            s.push_str(&format!("    new failures:  {}\n", trim_set(new_failures)));
            s.push_str(&format!("    still failing: {}\n", trim_set(still_failing)));
            s.push_str(&format!("    resolved:      {}\n", trim_set(resolved)));
            s
        }
        ExtractedDiff::Compile {
            same_location,
            prior_loc,
            curr_loc,
        } => {
            let same = if *same_location {
                "same location"
            } else {
                "different location"
            };
            format!("  Compile diff: {same} (prior: {prior_loc:?}, curr: {curr_loc:?})\n")
        }
        ExtractedDiff::Bind {
            same_port,
            prior_port,
            curr_port,
        } => {
            let same = if *same_port {
                "same port"
            } else {
                "different port"
            };
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

use crate::intelligence::VerificationVerb;

pub struct VerificationAffordance<'a> {
    pub todo_id: &'a str,
    pub title: &'a str,
    pub verb: VerificationVerb,
}

/// Render the cooperative-handoff section for any jobs the user is attached to.
pub fn attach_handoff_reminder(items: &[(JobView, Timestamp)]) -> String {
    let mut s = String::from("<system-reminder>\n");
    s.push_str("The user is currently attached to the following PTY jobs:\n");
    let tz = jiff::tz::TimeZone::system();
    for (j, attached_at) in items {
        // Local time render.
        let local = attached_at.to_zoned(tz.clone()).strftime("%H:%M");
        s.push_str(&format!(
            "- {} ({}) — attached at {} local\n",
            j.id.as_str(),
            j.description,
            local
        ));
    }
    s.push_str(
        "Defer stdin to the user. Do NOT call coding_task_stdin on these jobs while \
attached. You may still observe their output via coding_task_output. The \
attach indicator clears automatically when the user closes the panel.\n",
    );
    s.push_str("</system-reminder>");
    s
}

pub fn verification_affordance_reminder(items: &[VerificationAffordance<'_>]) -> String {
    let mut s = String::new();
    s.push_str("<system-reminder>\n");
    s.push_str("Plan mode active — the following pending TodoItems look like background-bash candidates after `/plan-exit`:\n");
    for item in items {
        s.push_str(&format!(
            "- \"{title}\" → bash(command=…, run_in_background=true) [verb: {verb}]\n",
            title = item.title,
            verb = item.verb.as_str(),
        ));
    }
    s.push_str("Background jobs cannot be spawned while plan mode is active. After ratification, you may launch these as background jobs.\n");
    s.push_str("</system-reminder>\n");
    s
}

#[cfg(test)]
mod verification_affordance_tests {
    use super::*;

    #[test]
    fn renders_each_item() {
        let items = [
            VerificationAffordance {
                todo_id: "t1",
                title: "Run integration tests",
                verb: VerificationVerb::Run,
            },
            VerificationAffordance {
                todo_id: "t2",
                title: "Verify migration safety",
                verb: VerificationVerb::Verify,
            },
        ];
        let body = verification_affordance_reminder(&items);
        assert!(body.contains("Plan mode active"));
        assert!(body.contains("Run integration tests"));
        assert!(body.contains("[verb: Run]"));
        assert!(body.contains("Verify migration safety"));
        assert!(body.contains("[verb: Verify]"));
    }
}

#[cfg(test)]
mod attach_render_tests {
    use super::*;
    use tools_core::{JobId, JobStatus, JobView};

    fn fake_view(id: &str, desc: &str) -> JobView {
        JobView {
            id: JobId(id.into()),
            session_id: "s1".into(),
            agent_id: "a1".into(),
            description: desc.into(),
            command: "c".into(),
            cwd: "/".into(),
            status: JobStatus::Running,
            started_at: jiff::Timestamp::now(),
            finished_at: None,
            exit_code: None,
            gate_result: None,
            failure_extracted: None,
            total_bytes_emitted: 0,
            bisect_generation: 0,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[test]
    fn renders_one_attached_job_with_handoff_text() {
        let v = fake_view("bash-aaaaaaaaaa", "gh auth login");
        let body = attach_handoff_reminder(&[(v, jiff::Timestamp::now())]);
        assert!(body.contains("<system-reminder>"));
        assert!(body.contains("bash-aaaaaaaaaa"));
        assert!(body.contains("gh auth login"));
        assert!(body.contains("Defer stdin to the user"));
        assert!(body.contains("Do NOT call coding_task_stdin"));
        assert!(body.contains("</system-reminder>"));
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
