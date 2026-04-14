//! Skill-driven salience resolver for entity events.
//!
//! Database SKILL.md files declare `salience.extract_on` / `salience.accumulate_on`
//! rules via [`skill_system::SalienceDeclaration`]. This module matches entity
//! events against those rules and returns the resolved verdict + importance.
//! When no rule matches (or no declaration exists), callers fall back to the
//! static defaults from [`crate::salience::evaluate_salience`].
//!
//! Rule matching is intentionally permissive: a rule with `field` set matches
//! when that field appears in `changed_fields`; a rule with `to_values` set
//! additionally requires the current field value to equal one of the listed
//! strings.

use std::collections::HashMap;

use serde_json::Value;
use skill_system::types::{SalienceDeclaration, SalienceRule};

use crate::types::SalienceVerdict;

/// Kind of entity event being evaluated. Distinguishes the default importance
/// applied when no rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityEventKind {
    Created,
    Updated,
    Deleted,
}

impl EntityEventKind {
    fn default_verdict_and_importance(self) -> (SalienceVerdict, f64) {
        match self {
            Self::Created => (SalienceVerdict::Accumulate, 0.3),
            Self::Updated => (SalienceVerdict::Accumulate, 0.2),
            Self::Deleted => (SalienceVerdict::Discard, 0.0),
        }
    }
}

/// Resolved salience for an entity event.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSalience {
    pub verdict: SalienceVerdict,
    pub importance: f64,
}

/// Resolve the verdict + importance for an entity event using optional skill rules.
///
/// Precedence: `extract_on` rules → Extract, else `accumulate_on` rules → Accumulate,
/// else default per [`EntityEventKind::default_verdict_and_importance`].
pub fn resolve_entity_salience(
    decl: Option<&SalienceDeclaration>,
    kind: EntityEventKind,
    changed_fields: &[String],
    field_values: &HashMap<String, Value>,
) -> ResolvedSalience {
    if let Some(decl) = decl {
        if let Some(rule) = match_rule(&decl.extract_on, changed_fields, field_values) {
            return ResolvedSalience {
                verdict: SalienceVerdict::Extract,
                importance: rule.importance.unwrap_or(0.8),
            };
        }
        if let Some(rule) = match_rule(&decl.accumulate_on, changed_fields, field_values) {
            let (_, default_importance) = kind.default_verdict_and_importance();
            return ResolvedSalience {
                verdict: SalienceVerdict::Accumulate,
                importance: rule.importance.unwrap_or(default_importance.max(0.1)),
            };
        }
    }
    let (verdict, importance) = kind.default_verdict_and_importance();
    ResolvedSalience {
        verdict,
        importance,
    }
}

/// Find the first rule whose field/to_values match the event context.
pub fn match_rule<'a>(
    rules: &'a [SalienceRule],
    changed_fields: &[String],
    field_values: &HashMap<String, Value>,
) -> Option<&'a SalienceRule> {
    rules
        .iter()
        .find(|r| rule_matches(r, changed_fields, field_values))
}

fn rule_matches(
    rule: &SalienceRule,
    changed_fields: &[String],
    field_values: &HashMap<String, Value>,
) -> bool {
    let Some(field) = rule.field.as_deref() else {
        // No field criterion — match iff no to_values either (degenerate catch-all rule).
        return rule.to_values.is_none();
    };
    // Field criterion: must appear in changed fields (or be present for Created events).
    let field_changed = changed_fields.iter().any(|f| f == field);
    if !field_changed && !changed_fields.is_empty() {
        return false;
    }
    // Value criterion: current value must be one of to_values (string-compare).
    if let Some(to_values) = rule.to_values.as_ref() {
        let current = field_values.get(field).and_then(|v| value_as_string(v));
        match current {
            Some(ref s) => to_values.iter().any(|v: &String| v == s),
            None => false,
        }
    } else {
        true
    }
}

fn value_as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(field: &str, to_values: Option<Vec<&str>>, importance: Option<f64>) -> SalienceRule {
        SalienceRule {
            field: Some(field.to_string()),
            event: None,
            to_values: to_values.map(|v| v.into_iter().map(String::from).collect()),
            importance,
        }
    }

    #[test]
    fn no_declaration_falls_back_to_default() {
        let out = resolve_entity_salience(None, EntityEventKind::Created, &[], &HashMap::new());
        assert_eq!(out.verdict, SalienceVerdict::Accumulate);
        assert!((out.importance - 0.3).abs() < 1e-9);

        let out = resolve_entity_salience(None, EntityEventKind::Deleted, &[], &HashMap::new());
        assert_eq!(out.verdict, SalienceVerdict::Discard);
    }

    #[test]
    fn rule_matches_field_and_values() {
        let decl = SalienceDeclaration {
            extract_on: vec![rule("status", Some(vec!["done"]), Some(0.9))],
            accumulate_on: vec![],
        };
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), json!("done"));

        let out = resolve_entity_salience(
            Some(&decl),
            EntityEventKind::Updated,
            &["status".to_string()],
            &fields,
        );
        assert_eq!(out.verdict, SalienceVerdict::Extract);
        assert!((out.importance - 0.9).abs() < 1e-9);
    }

    #[test]
    fn value_mismatch_skips_rule() {
        let decl = SalienceDeclaration {
            extract_on: vec![rule("status", Some(vec!["done"]), Some(0.9))],
            accumulate_on: vec![],
        };
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), json!("in_progress"));

        let out = resolve_entity_salience(
            Some(&decl),
            EntityEventKind::Updated,
            &["status".to_string()],
            &fields,
        );
        assert_eq!(out.verdict, SalienceVerdict::Accumulate);
        assert!((out.importance - 0.2).abs() < 1e-9);
    }

    #[test]
    fn extract_rule_overrides_accumulate() {
        let decl = SalienceDeclaration {
            extract_on: vec![rule("priority", Some(vec!["high"]), Some(0.95))],
            accumulate_on: vec![rule("priority", None, Some(0.5))],
        };
        let mut fields = HashMap::new();
        fields.insert("priority".to_string(), json!("high"));

        let out = resolve_entity_salience(
            Some(&decl),
            EntityEventKind::Updated,
            &["priority".to_string()],
            &fields,
        );
        assert_eq!(out.verdict, SalienceVerdict::Extract);
        assert!((out.importance - 0.95).abs() < 1e-9);
    }

    #[test]
    fn accumulate_rule_applies_custom_importance() {
        let decl = SalienceDeclaration {
            extract_on: vec![],
            accumulate_on: vec![rule("tag", None, Some(0.4))],
        };
        let out = resolve_entity_salience(
            Some(&decl),
            EntityEventKind::Updated,
            &["tag".to_string()],
            &HashMap::new(),
        );
        assert_eq!(out.verdict, SalienceVerdict::Accumulate);
        assert!((out.importance - 0.4).abs() < 1e-9);
    }
}
