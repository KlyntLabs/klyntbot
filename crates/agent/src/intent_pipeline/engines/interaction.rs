use cognitive::PersonaRow;

use super::debate_types::*;

const GROUP_TRIGGERS: &[&str] = &["everyone", "squad", "team", "all of you"];

/// Detects how a message should be routed within a squad session.
///
/// Priority:
/// 1. Group triggers ("everyone", "squad", "team") → Debate
/// 2. @PersonaName anywhere → DirectAddress
/// 3. "PersonaName," or "PersonaName:" at message start → DirectAddress
/// 4. Fallback to squad's default_interaction_mode
///
/// Name matching is case-insensitive, full exact name only (no substring).
pub fn detect_interaction_mode(
    message: &str,
    personas: &[PersonaRow],
    default: DefaultInteractionMode,
) -> SquadInteractionMode {
    let msg_lower = message.to_lowercase();
    let msg_trimmed = msg_lower.trim();

    // 1. Check for group triggers at message start
    for trigger in GROUP_TRIGGERS {
        if msg_trimmed.starts_with(trigger) {
            return SquadInteractionMode::Debate;
        }
    }

    // 2. Check for @PersonaName anywhere
    for persona in personas {
        let at_name = format!("@{}", persona.name.to_lowercase());
        if msg_lower.contains(&at_name) {
            return SquadInteractionMode::DirectAddress {
                persona_id: persona.id.clone(),
            };
        }
    }

    // 3. Check for "PersonaName," or "PersonaName:" at message start
    for persona in personas {
        let name_lower = persona.name.to_lowercase();
        let comma_pattern = format!("{},", name_lower);
        let colon_pattern = format!("{}:", name_lower);
        if msg_trimmed.starts_with(&comma_pattern) || msg_trimmed.starts_with(&colon_pattern) {
            return SquadInteractionMode::DirectAddress {
                persona_id: persona.id.clone(),
            };
        }
    }

    // 4. Fallback to default mode
    match default {
        DefaultInteractionMode::Lead | DefaultInteractionMode::Smart => {
            SquadInteractionMode::LeadResponse
        }
        DefaultInteractionMode::Debate => SquadInteractionMode::Debate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_persona(id: &str, name: &str) -> PersonaRow {
        PersonaRow {
            id: id.to_string(),
            name: name.to_string(),
            role: "Test".to_string(),
            expertise: String::new(),
            perspective: String::new(),
            tone: "analytical".to_string(),
            icon: "🧪".to_string(),
            source: "builtin".to_string(),
            domains: "[]".to_string(),
            is_active: 1,
            relevance_score: 0.5,
            skill_path: None,
            questioning_style: "analytical".to_string(),
            cognitive_bias: "balanced".to_string(),
            analysis_frameworks: "[]".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn test_personas() -> Vec<PersonaRow> {
        vec![
            make_persona("p1", "Skeptic"),
            make_persona("p2", "Strategist"),
            make_persona("p3", "Deep Analyst"),
        ]
    }

    #[test]
    fn test_detect_group_trigger_everyone() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "everyone, what do you think?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::Debate,
        );
    }

    #[test]
    fn test_detect_group_trigger_team() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "team, weigh in on this",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::Debate,
        );
    }

    #[test]
    fn test_detect_direct_address_at_sign() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "@Skeptic what about taxes?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p1".into()
            },
        );
    }

    #[test]
    fn test_detect_direct_address_at_sign_mid_message() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "I think @Strategist should weigh in",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p2".into()
            },
        );
    }

    #[test]
    fn test_detect_direct_address_comma() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "Skeptic, what about taxes?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p1".into()
            },
        );
    }

    #[test]
    fn test_detect_direct_address_colon() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "Skeptic: what about taxes?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p1".into()
            },
        );
    }

    #[test]
    fn test_detect_multi_word_name() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "Deep Analyst, break this down",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p3".into()
            },
        );
    }

    #[test]
    fn test_no_substring_match() {
        let personas = test_personas();
        // "skeptical" should NOT match "Skeptic"
        assert_eq!(
            detect_interaction_mode(
                "Can you be more skeptical about this?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::LeadResponse,
        );
    }

    #[test]
    fn test_fallback_to_lead() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "What do you think about this?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::LeadResponse,
        );
    }

    #[test]
    fn test_fallback_to_debate() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "What do you think about this?",
                &personas,
                DefaultInteractionMode::Debate
            ),
            SquadInteractionMode::Debate,
        );
    }

    #[test]
    fn test_case_insensitive() {
        let personas = test_personas();
        assert_eq!(
            detect_interaction_mode(
                "@skeptic what about taxes?",
                &personas,
                DefaultInteractionMode::Lead
            ),
            SquadInteractionMode::DirectAddress {
                persona_id: "p1".into()
            },
        );
    }
}
