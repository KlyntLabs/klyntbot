//! Typed helpers for parsing cognitive facts into task-domain structures.
//!
//! These functions bridge the cognitive memory system with task-domain types
//! (EnergyProfile, estimation bias, velocity, deferral patterns, etc.).
//! Pure computation — no LLM calls, no I/O.
//!
//! Uses a local `CognitiveFact` struct instead of importing from the `cognitive`
//! crate to avoid an L4→L5 layer violation. Callers convert at the call site.

/// Minimal fact representation matching the fields cognitive_bridge needs.
///
/// Callers convert from `cognitive::types::SemanticFact` at the call site.
#[derive(Debug, Clone)]
pub struct CognitiveFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// A parsed energy profile from cognitive facts.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEnergyProfile {
    pub peak_hours: Option<String>,
    pub preferred_energy_by_period: Vec<(String, String)>,
}

/// Extract an energy profile from cognitive facts.
///
/// Looks for facts with predicates: `peak_focus_hours`, `preferred_energy_*`.
pub fn extract_energy_profile(facts: &[CognitiveFact]) -> Option<ParsedEnergyProfile> {
    let peak = facts
        .iter()
        .find(|f| f.predicate == "peak_focus_hours")
        .map(|f| f.object.clone());

    let prefs: Vec<(String, String)> = facts
        .iter()
        .filter(|f| f.predicate.starts_with("preferred_energy_"))
        .map(|f| {
            let period = f
                .predicate
                .strip_prefix("preferred_energy_")
                .unwrap_or("")
                .to_string();
            (period, f.object.clone())
        })
        .collect();

    if peak.is_some() || !prefs.is_empty() {
        Some(ParsedEnergyProfile {
            peak_hours: peak,
            preferred_energy_by_period: prefs,
        })
    } else {
        None
    }
}

/// Extract estimation bias from cognitive facts.
///
/// Looks for `estimation_bias` (general) and `estimation_bias_{tag}` (per-tag).
/// Returns the bias as a fraction (e.g., 0.38 = +38% underestimation).
pub fn extract_estimation_bias(facts: &[CognitiveFact], tags: &[String]) -> Option<f64> {
    // Try tag-specific bias first
    for tag in tags {
        let predicate = format!("estimation_bias_{tag}");
        if let Some(fact) = facts.iter().find(|f| f.predicate == predicate) {
            if let Some(bias) = parse_bias_value(&fact.object) {
                return Some(bias);
            }
        }
    }

    // Fall back to general bias
    facts
        .iter()
        .find(|f| f.predicate == "estimation_bias")
        .and_then(|f| parse_bias_value(&f.object))
}

/// Extract task completion velocity from cognitive facts.
///
/// Looks for `completion_pace` (per-project) or `tasks_completed_per_week` (global).
pub fn extract_velocity(facts: &[CognitiveFact], project_id: Option<&str>) -> Option<f64> {
    // Try project-specific velocity first
    if let Some(pid) = project_id {
        let subject = format!("project:{pid}");
        if let Some(fact) = facts
            .iter()
            .find(|f| f.subject == subject && f.predicate == "completion_pace")
        {
            if let Some(v) = parse_numeric_value(&fact.object) {
                return Some(v);
            }
        }
    }

    // Fall back to global velocity
    facts
        .iter()
        .find(|f| f.predicate == "tasks_completed_per_week")
        .and_then(|f| parse_numeric_value(&f.object))
}

/// Extract deferral patterns from cognitive facts.
///
/// Looks for facts with predicate `deferral_pattern`.
pub fn extract_deferral_patterns(facts: &[CognitiveFact]) -> Vec<String> {
    facts
        .iter()
        .filter(|f| f.predicate == "deferral_pattern")
        .map(|f| f.object.clone())
        .collect()
}

/// Extract agentic task success rate from cognitive facts.
///
/// Looks for `agentic_success_rate` predicate. Returns as fraction (e.g., 0.78 = 78%).
pub fn extract_agentic_success_rate(facts: &[CognitiveFact]) -> Option<f64> {
    facts
        .iter()
        .find(|f| f.predicate == "agentic_success_rate")
        .and_then(|f| parse_percentage_value(&f.object))
}

/// Parse a bias string like "+38% underestimation" or "-10%" into a fraction.
fn parse_bias_value(s: &str) -> Option<f64> {
    // Strip everything after '%' (if present), then parse the leading number
    let stripped = s.split('%').next().unwrap_or(s);
    parse_numeric_value(stripped).map(|v| v / 100.0)
}

/// Parse a numeric value from strings like "12.5 average" or "3.2 tasks/week".
fn parse_numeric_value(s: &str) -> Option<f64> {
    s.split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
}

/// Parse a percentage string like "78% (7/9)" or "0.78" into a fraction.
fn parse_percentage_value(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    // Try "78% ..." format — only if string actually contains '%'
    if trimmed.contains('%') {
        if let Some(pct_str) = trimmed.split('%').next() {
            if let Ok(v) = pct_str.trim().parse::<f64>() {
                return Some(v / 100.0);
            }
        }
    }
    // Try raw fraction "0.78"
    trimmed
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v >= 0.0 && *v <= 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(subject: &str, predicate: &str, object: &str) -> CognitiveFact {
        CognitiveFact {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }

    #[test]
    fn test_extract_energy_profile_with_peak() {
        let facts = vec![
            fact("user", "peak_focus_hours", "9:00-11:30"),
            fact("user", "preferred_energy_morning", "deep"),
            fact("user", "preferred_energy_afternoon", "medium"),
        ];
        let profile = extract_energy_profile(&facts).unwrap();
        assert_eq!(profile.peak_hours, Some("9:00-11:30".into()));
        assert_eq!(profile.preferred_energy_by_period.len(), 2);
    }

    #[test]
    fn test_extract_energy_profile_none() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_energy_profile(&facts).is_none());
    }

    #[test]
    fn test_extract_estimation_bias_general() {
        let facts = vec![fact("user", "estimation_bias", "+38% underestimation")];
        let bias = extract_estimation_bias(&facts, &[]).unwrap();
        assert!((bias - 0.38).abs() < 0.01);
    }

    #[test]
    fn test_extract_estimation_bias_tag_specific() {
        let facts = vec![
            fact("user", "estimation_bias", "+38% underestimation"),
            fact("user", "estimation_bias_rust", "+55% for rust tasks"),
        ];
        let bias = extract_estimation_bias(&facts, &["rust".into()]).unwrap();
        assert!((bias - 0.55).abs() < 0.01);
    }

    #[test]
    fn test_extract_estimation_bias_none() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_estimation_bias(&facts, &[]).is_none());
    }

    #[test]
    fn test_extract_velocity_project() {
        let facts = vec![
            fact("project:p1", "completion_pace", "3.2 tasks/week"),
            fact("user", "tasks_completed_per_week", "12.5 average"),
        ];
        let v = extract_velocity(&facts, Some("p1")).unwrap();
        assert!((v - 3.2).abs() < 0.01);
    }

    #[test]
    fn test_extract_velocity_global_fallback() {
        let facts = vec![fact("user", "tasks_completed_per_week", "12.5 average")];
        let v = extract_velocity(&facts, Some("p999")).unwrap();
        assert!((v - 12.5).abs() < 0.01);
    }

    #[test]
    fn test_extract_deferral_patterns() {
        let facts = vec![
            fact("user", "deferral_pattern", "defers planning tasks to someday"),
            fact("user", "deferral_pattern", "defers research tasks when busy"),
        ];
        let patterns = extract_deferral_patterns(&facts);
        assert_eq!(patterns.len(), 2);
    }

    #[test]
    fn test_extract_agentic_success_rate() {
        let facts = vec![fact("user", "agentic_success_rate", "78% (7/9)")];
        let rate = extract_agentic_success_rate(&facts).unwrap();
        assert!((rate - 0.78).abs() < 0.01);
    }

    #[test]
    fn test_parse_bias_negative() {
        assert!((parse_bias_value("-10%").unwrap() - (-0.10)).abs() < 0.01);
    }

    #[test]
    fn test_parse_percentage_raw_fraction() {
        assert!((parse_percentage_value("0.78").unwrap() - 0.78).abs() < 0.01);
    }

    #[test]
    fn test_extract_velocity_none_when_no_facts() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_velocity(&facts, Some("p1")).is_none());
    }

    #[test]
    fn test_extract_agentic_success_rate_none() {
        let facts = vec![fact("user", "favorite_color", "blue")];
        assert!(extract_agentic_success_rate(&facts).is_none());
    }
}
