//! Voice routing — suggests skill targets from partial transcripts.

use crate::events::VoiceEvent;

const ROUTING_THRESHOLD: f64 = 0.4;

#[derive(Debug, Clone)]
pub struct DetectedIntent {
    pub skill: String,
    pub confidence: f64,
    pub label: String,
    pub trigger_text: String,
}

struct SkillRoute {
    skill: String,
    label: String,
    keywords: Vec<String>,
}

pub struct VoiceRouter {
    skill_keywords: Vec<SkillRoute>,
}

impl VoiceRouter {
    pub fn new() -> Self {
        Self {
            skill_keywords: vec![
                SkillRoute {
                    skill: "tasks".into(),
                    label: "Task".into(),
                    // Two-keyword set: any single hit = 0.5 >= threshold.
                    // "remind" covers "remind/reminder"; "schedule" covers scheduling.
                    keywords: vec!["remind", "schedule"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                SkillRoute {
                    skill: "learning".into(),
                    label: "Learning".into(),
                    // Two-keyword set: any single hit = 0.5 >= threshold.
                    // "practice" covers drills; "vocab" covers vocab/vocabulary via contains.
                    keywords: vec!["practice", "vocab"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                SkillRoute {
                    skill: "notes".into(),
                    label: "Note".into(),
                    keywords: vec!["note", "jot"].into_iter().map(String::from).collect(),
                },
                SkillRoute {
                    skill: "finance".into(),
                    label: "Finance".into(),
                    keywords: vec!["budget", "expense"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
            ],
        }
    }

    pub fn detect_intents(&self, text: &str) -> Vec<DetectedIntent> {
        let words: Vec<String> = text
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        self.skill_keywords
            .iter()
            .filter_map(|route| {
                let hits = route
                    .keywords
                    .iter()
                    .filter(|kw| words.iter().any(|w| w.contains(kw.as_str())))
                    .count();
                if hits == 0 {
                    return None;
                }

                let score = (hits as f64 / route.keywords.len().max(1) as f64).min(1.0);
                if score >= ROUTING_THRESHOLD {
                    Some(DetectedIntent {
                        skill: route.skill.clone(),
                        confidence: score,
                        label: format!("→ {}", route.label),
                        trigger_text: text.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn to_events(&self, intents: &[DetectedIntent]) -> Vec<VoiceEvent> {
        intents
            .iter()
            .map(|intent| VoiceEvent::RoutingSuggestion {
                skill: intent.skill.clone(),
                confidence: intent.confidence as f32,
                label: intent.label.clone(),
            })
            .collect()
    }

    pub fn is_multi_intent(intents: &[DetectedIntent]) -> bool {
        intents.len() >= 2
    }
}

impl Default for VoiceRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_task_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("remind me to schedule dentist");
        assert!(!intents.is_empty());
        assert_eq!(intents[0].skill, "tasks");
    }

    #[test]
    fn detects_learning_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("practice french vocabulary");
        assert!(!intents.is_empty());
        assert_eq!(intents[0].skill, "learning");
    }

    #[test]
    fn detects_multi_intent() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("schedule dentist and practice french vocab");
        assert!(intents.len() >= 2);
        let skills: Vec<&str> = intents.iter().map(|i| i.skill.as_str()).collect();
        assert!(skills.contains(&"tasks"));
        assert!(skills.contains(&"learning"));
        assert!(VoiceRouter::is_multi_intent(&intents));
    }

    #[test]
    fn no_intent_from_generic_text() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("hello how are you");
        assert!(intents.is_empty());
    }

    #[test]
    fn single_intent_is_not_multi() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("add a task for tomorrow");
        assert!(!VoiceRouter::is_multi_intent(&intents));
    }

    #[test]
    fn to_events_produces_routing_suggestions() {
        let router = VoiceRouter::new();
        let intents = router.detect_intents("remind me to practice french");
        let events = router.to_events(&intents);
        assert!(events
            .iter()
            .all(|e| matches!(e, VoiceEvent::RoutingSuggestion { .. })));
    }
}
