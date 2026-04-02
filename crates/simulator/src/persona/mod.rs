pub mod templates;
pub mod types;

pub use types::*;

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Duration, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use templates::{
    fill_template, pick_template, templates_for_topic, CORRECTION_TEMPLATES,
    FACT_INTRODUCTION_TEMPLATES,
};

// ── PersonaRunner ─────────────────────────────────────────────────────

/// Drives a single [`Persona`] through its lifecycle phases, generating
/// deterministic daily message batches with ground-truth annotations.
pub struct PersonaRunner {
    persona: Persona,
    current_phase: LifecyclePhase,
    day_in_phase: u32,
    rng: StdRng,
    introduced_facts: HashSet<String>,
    topic_history: VecDeque<String>,
    created_task_titles: Vec<String>,
    extracted_fact_ids_by_topic: HashMap<String, Vec<String>>,
}

impl PersonaRunner {
    /// Create a new runner seeded from the persona's seed value.
    pub fn new(persona: Persona) -> Self {
        let rng = StdRng::seed_from_u64(persona.seed);
        Self {
            persona,
            current_phase: LifecyclePhase::Onboarding,
            day_in_phase: 0,
            rng,
            introduced_facts: HashSet::new(),
            topic_history: VecDeque::new(),
            created_task_titles: Vec::new(),
            extracted_fact_ids_by_topic: HashMap::new(),
        }
    }

    pub fn current_phase(&self) -> LifecyclePhase {
        self.current_phase
    }

    pub fn persona(&self) -> &Persona {
        &self.persona
    }

    /// Return the [`PhaseConfig`] for the current lifecycle phase.
    pub fn phase_config(&self) -> &PhaseConfig {
        match self.current_phase {
            LifecyclePhase::Onboarding => &self.persona.phases.onboarding,
            LifecyclePhase::Routine => &self.persona.phases.routine,
            LifecyclePhase::PowerUser => &self.persona.phases.power_user,
            LifecyclePhase::BehaviorShift => &self.persona.phases.behavior_shift,
        }
    }

    /// Return the configured messages-per-day for the current phase.
    pub fn messages_per_day(&self) -> u32 {
        match self.current_phase {
            LifecyclePhase::Onboarding => self.persona.messages_per_day.onboarding,
            LifecyclePhase::Routine => self.persona.messages_per_day.routine,
            LifecyclePhase::PowerUser => self.persona.messages_per_day.power_user,
            LifecyclePhase::BehaviorShift => self.persona.messages_per_day.shift,
        }
    }

    /// Record an extracted fact ID associated with a topic, for later
    /// retrieval annotation backfill.
    pub fn record_extracted_fact(&mut self, topic: &str, fact_id: &str) {
        self.extracted_fact_ids_by_topic
            .entry(topic.to_string())
            .or_default()
            .push(fact_id.to_string());
    }

    /// Return the most recent (up to 5) extracted fact IDs for the given topic.
    pub fn relevant_facts_for_topic(&self, topic: &str) -> Vec<String> {
        self.extracted_fact_ids_by_topic
            .get(topic)
            .map(|ids| ids.iter().rev().take(5).cloned().collect())
            .unwrap_or_default()
    }

    /// Advance the phase if the current phase's duration has been exceeded.
    /// `BehaviorShift` is the terminal phase — it never transitions further.
    pub fn check_phase_transition(&mut self) {
        let duration = self.phase_config().duration_days;
        if self.day_in_phase >= duration {
            let next = match self.current_phase {
                LifecyclePhase::Onboarding => LifecyclePhase::Routine,
                LifecyclePhase::Routine => LifecyclePhase::PowerUser,
                LifecyclePhase::PowerUser => LifecyclePhase::BehaviorShift,
                LifecyclePhase::BehaviorShift => return, // stay
            };
            self.current_phase = next;
            self.day_in_phase = 0;
        }
    }

    /// Weighted random topic selection from the current phase's topic_weights,
    /// with a sliding-window dedup (retries up to 5 times if the last 3 topics
    /// are all the same as the candidate).
    pub fn sample_topic(&mut self) -> String {
        let mut weights: Vec<(String, f64)> = self
            .phase_config()
            .topic_weights
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        // Sort by key for deterministic iteration order (HashMap is unordered).
        weights.sort_by(|(a, _), (b, _)| a.cmp(b));

        let total_weight: f64 = weights.iter().map(|(_, w)| w).sum();

        for _ in 0..5 {
            let mut roll = self.rng.random::<f64>() * total_weight;
            let mut chosen = weights.last().map(|(k, _)| k.clone()).unwrap_or_default();

            for (topic, weight) in &weights {
                roll -= weight;
                if roll <= 0.0 {
                    chosen = topic.clone();
                    break;
                }
            }

            // Dedup check: skip if the last 3 entries in the history are all the same
            // as the candidate.
            let recent: Vec<&String> = self.topic_history.iter().rev().take(3).collect();
            if recent.len() >= 3 && recent.iter().all(|t| **t == chosen) {
                continue;
            }

            return chosen;
        }

        // Fallback: return the first topic alphabetically.
        let mut keys: Vec<String> = self.phase_config().topic_weights.keys().cloned().collect();
        keys.sort();
        keys.into_iter()
            .next()
            .unwrap_or_else(|| "chat".to_string())
    }

    /// Generate a [`SimulatedToolAction`] appropriate for the given topic.
    /// Returns `None` for topics that don't map to a tool action.
    pub fn generate_tool_action(
        &mut self,
        topic: &str,
        _simulated_at: DateTime<Utc>,
    ) -> Option<SimulatedToolAction> {
        match topic {
            "tasks" => {
                // 30% chance to complete an existing task instead of creating a new one
                if !self.created_task_titles.is_empty() && self.rng.random::<f64>() < 0.3 {
                    let idx = self.rng.random_range(0..self.created_task_titles.len());
                    Some(SimulatedToolAction::CompleteTask {
                        task_ref: self.created_task_titles[idx].clone(),
                    })
                } else {
                    let titles = [
                        "review PR",
                        "update docs",
                        "fix bug in parser",
                        "write tests",
                        "deploy staging",
                        "plan sprint",
                    ];
                    let title = titles[self.rng.random_range(0..titles.len())].to_string();
                    self.created_task_titles.push(title.clone());
                    Some(SimulatedToolAction::CreateTask {
                        title,
                        due_offset_days: Some(self.rng.random_range(1..8)),
                        project: Some("main".to_string()),
                    })
                }
            }
            "notes" => Some(SimulatedToolAction::CreateNote {
                title: format!("note-{}", self.rng.random_range(1..1000u32)),
                content: "Some captured thought".to_string(),
            }),
            "finance" => {
                let categories = ["food", "transport", "software", "office"];
                let cat = categories[self.rng.random_range(0..categories.len())];
                Some(SimulatedToolAction::RecordTransaction {
                    amount: (self.rng.random_range(5..200u32)) as f64,
                    category: cat.to_string(),
                    description: format!("{cat} expense"),
                })
            }
            "productivity" => Some(SimulatedToolAction::StartFocus {
                task_ref: self.created_task_titles.last().cloned(),
                duration_mins: self.rng.random_range(15..60),
            }),
            _ => None,
        }
    }

    /// Generate all messages for a single simulated day.
    ///
    /// 1. Check phase transition
    /// 2. Sample message count (messages_per_day ± 2, min 1)
    /// 3. For each message: spread across 9am-9pm, sample topic, optionally
    ///    inject a correction or fact introduction, generate content from templates
    /// 4. Increment day_in_phase
    /// 5. Return the batch
    pub fn generate_day(&mut self, simulated_date: DateTime<Utc>) -> Vec<AnnotatedMessage> {
        // 1. Phase transition check.
        self.check_phase_transition();

        let phase = self.current_phase;
        let config = self.phase_config().clone();

        // 2. Sample message count.
        let base = self.messages_per_day() as i32;
        let jitter = self.rng.random_range(-2..=2i32);
        let count = (base + jitter).max(1) as u32;

        let mut messages = Vec::with_capacity(count as usize);

        for i in 0..count {
            // 3a. Spread messages across 9am-9pm (12 hour window).
            let hour_offset = if count > 1 {
                (12 * i) / (count - 1)
            } else {
                6 // noon
            };
            let msg_time = simulated_date + Duration::hours(9 + hour_offset as i64);

            // 3b. Sample topic.
            let topic = self.sample_topic();

            // Push to history, maintain sliding window of 20.
            self.topic_history.push_back(topic.clone());
            if self.topic_history.len() > 20 {
                self.topic_history.pop_front();
            }

            // 3c. Decide if this is a correction.
            let is_correction = self.rng.random::<f64>() < config.correction_rate;

            // 3d. Decide if we introduce a fact.
            let fact_to_introduce = if self.rng.random::<f64>() < config.new_fact_introduction_rate
            {
                self.pick_unintroduced_fact()
            } else {
                None
            };

            // 3e. Decide if we generate a tool action.
            let tool_action = if self.rng.random::<f64>() < config.tool_action_rate {
                self.generate_tool_action(&topic, msg_time)
            } else {
                None
            };

            // 3f. Generate message content.
            let (content, ground_truth) = if let Some(ref fact) = fact_to_introduce {
                // Fact introduction message.
                let template = pick_template(FACT_INTRODUCTION_TEMPLATES, &mut self.rng);
                let vars = [
                    ("predicate", fact.predicate.as_str()),
                    ("object", fact.object.as_str()),
                ];
                let text = fill_template(template, &vars);
                let gt = GroundTruthAnnotation {
                    introduces_fact: Some(fact.clone()),
                    relevant_facts: vec![],
                    expected_skill: None,
                };
                (text, Some(gt))
            } else if is_correction {
                // Correction message.
                let template = pick_template(CORRECTION_TEMPLATES, &mut self.rng);
                let vars = [
                    ("correct_value", "the right thing"),
                    ("wrong_value", "the wrong thing"),
                ];
                let text = fill_template(template, &vars);
                (text, None)
            } else {
                // Normal topic message.
                let templates = templates_for_topic(&topic);
                let template = pick_template(templates, &mut self.rng);
                let owned_vars = self.default_vars_for_topic(&topic);
                let vars: Vec<(&str, &str)> =
                    owned_vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
                let text = fill_template(template, &vars);
                let gt = GroundTruthAnnotation {
                    introduces_fact: None,
                    relevant_facts: vec![],
                    expected_skill: self.expected_skill_for_topic(&topic),
                };
                (text, Some(gt))
            };

            let tool_actions = tool_action.into_iter().collect();

            messages.push(AnnotatedMessage {
                content,
                phase,
                simulated_at: msg_time,
                ground_truth,
                tool_actions,
                is_correction,
                topic,
            });
        }

        // Guarantee all facts are introduced by the last day of each phase.
        let is_last_day_of_phase = self.day_in_phase + 1 >= config.duration_days;
        if is_last_day_of_phase {
            while let Some(fact) = self.pick_unintroduced_fact() {
                let template = pick_template(FACT_INTRODUCTION_TEMPLATES, &mut self.rng);
                let vars = [
                    ("predicate", fact.predicate.as_str()),
                    ("object", fact.object.as_str()),
                ];
                let text = fill_template(template, &vars);
                let gt = GroundTruthAnnotation {
                    introduces_fact: Some(fact.clone()),
                    relevant_facts: vec![],
                    expected_skill: None,
                };
                let msg_time = simulated_date + Duration::hours(20);
                messages.push(AnnotatedMessage {
                    content: text,
                    phase,
                    simulated_at: msg_time,
                    ground_truth: Some(gt),
                    tool_actions: vec![],
                    is_correction: false,
                    topic: "chat".to_string(),
                });
            }
        }

        // 4. Increment day_in_phase.
        self.day_in_phase += 1;

        messages
    }

    // ── Private helpers ───────────────────────────────────────────────

    /// Pick an unintroduced fact from `known_facts` + current phase's `new_facts`,
    /// mark it as introduced, and return it.
    fn pick_unintroduced_fact(&mut self) -> Option<FactTriple> {
        let mut candidates: Vec<FactTriple> = self.persona.profile.known_facts.clone();
        candidates.extend(self.phase_config().new_facts.clone());

        let unintroduced: Vec<FactTriple> = candidates
            .into_iter()
            .filter(|f| {
                let key = format!("{}:{}:{}", f.subject, f.predicate, f.object);
                !self.introduced_facts.contains(&key)
            })
            .collect();

        if unintroduced.is_empty() {
            return None;
        }

        let idx = self.rng.random_range(0..unintroduced.len());
        let fact = unintroduced[idx].clone();
        let key = format!("{}:{}:{}", fact.subject, fact.predicate, fact.object);
        self.introduced_facts.insert(key);
        Some(fact)
    }

    /// Return default template variable bindings for a given topic.
    fn default_vars_for_topic(&mut self, topic: &str) -> Vec<(&'static str, String)> {
        match topic {
            "tasks" => {
                let actions = ["review PR", "update docs", "fix parser bug", "write tests"];
                let action = actions[self.rng.random_range(0..actions.len())];
                vec![
                    ("action", action.to_string()),
                    ("project", "main project".to_string()),
                    ("due_date", "Friday".to_string()),
                    (
                        "task",
                        self.created_task_titles
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "my task".to_string()),
                    ),
                ]
            }
            "notes" => vec![
                ("topic", "meeting notes".to_string()),
                ("content", "key decisions from today".to_string()),
            ],
            "finance" => vec![
                ("amount", format!("${}", self.rng.random_range(10..500u32))),
                ("category", "software".to_string()),
                ("description", "monthly subscription".to_string()),
            ],
            "productivity" => vec![
                (
                    "task",
                    self.created_task_titles
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "my task".to_string()),
                ),
                ("energy", "focused".to_string()),
            ],
            "automation" => vec![
                ("action", "standup check".to_string()),
                ("time", "9am".to_string()),
                ("frequency", "weekday".to_string()),
            ],
            "learning" => vec![
                ("topic", "Rust".to_string()),
                ("content", "lifetimes are lexical in older Rust".to_string()),
            ],
            _ => vec![],
        }
    }

    /// Map topic to expected skill name for ground-truth annotations.
    fn expected_skill_for_topic(&self, topic: &str) -> Option<String> {
        match topic {
            "tasks" => Some("task-management".to_string()),
            "notes" => Some("general".to_string()),
            "finance" => Some("finance-management".to_string()),
            "productivity" => Some("general".to_string()),
            "automation" => Some("automation".to_string()),
            "learning" => Some("general".to_string()),
            "insights" => Some("general".to_string()),
            _ => None,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Build a minimal test persona.
    fn test_persona() -> Persona {
        Persona {
            name: "test_user".to_string(),
            timezone: "UTC".to_string(),
            language: "en".to_string(),
            seed: 42,
            messages_per_day: MessagesPerDay {
                onboarding: 5,
                routine: 3,
                power_user: 4,
                shift: 3,
            },
            profile: PersonaProfile {
                known_facts: vec![
                    FactTriple {
                        subject: "user".to_string(),
                        predicate: "works_as".to_string(),
                        object: "engineer".to_string(),
                    },
                    FactTriple {
                        subject: "user".to_string(),
                        predicate: "lives_in".to_string(),
                        object: "Berlin".to_string(),
                    },
                ],
            },
            phases: PersonaPhases {
                onboarding: PhaseConfig {
                    duration_days: 3,
                    correction_rate: 0.2,
                    topic_weights: HashMap::from([
                        ("tasks".to_string(), 0.4),
                        ("chat".to_string(), 0.4),
                        ("notes".to_string(), 0.2),
                    ]),
                    new_fact_introduction_rate: 0.5,
                    tool_action_rate: 0.3,
                    shift_description: None,
                    new_facts: vec![],
                },
                routine: PhaseConfig {
                    duration_days: 7,
                    correction_rate: 0.1,
                    topic_weights: HashMap::from([
                        ("tasks".to_string(), 0.5),
                        ("chat".to_string(), 0.3),
                        ("finance".to_string(), 0.2),
                    ]),
                    new_fact_introduction_rate: 0.1,
                    tool_action_rate: 0.5,
                    shift_description: None,
                    new_facts: vec![],
                },
                power_user: PhaseConfig {
                    duration_days: 7,
                    correction_rate: 0.05,
                    topic_weights: HashMap::from([
                        ("tasks".to_string(), 0.3),
                        ("notes".to_string(), 0.3),
                        ("productivity".to_string(), 0.2),
                        ("finance".to_string(), 0.2),
                    ]),
                    new_fact_introduction_rate: 0.05,
                    tool_action_rate: 0.7,
                    shift_description: None,
                    new_facts: vec![],
                },
                behavior_shift: PhaseConfig {
                    duration_days: 7,
                    correction_rate: 0.15,
                    topic_weights: HashMap::from([
                        ("tasks".to_string(), 0.3),
                        ("notes".to_string(), 0.4),
                        ("chat".to_string(), 0.3),
                    ]),
                    new_fact_introduction_rate: 0.4,
                    tool_action_rate: 0.5,
                    shift_description: Some("switches to data science".to_string()),
                    new_facts: vec![FactTriple {
                        subject: "user".to_string(),
                        predicate: "learning".to_string(),
                        object: "Python".to_string(),
                    }],
                },
            },
        }
    }

    fn base_date() -> DateTime<Utc> {
        chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 3, 1, 0, 0, 0).unwrap()
    }

    #[test]
    fn generates_messages_for_day() {
        let persona = test_persona();
        let mut runner = PersonaRunner::new(persona);

        let messages = runner.generate_day(base_date());

        assert!(!messages.is_empty(), "should generate at least 1 message");
        // Base is 5 for onboarding ± 2 → range [3, 7].
        assert!(
            messages.len() >= 3 && messages.len() <= 7,
            "expected 3-7 messages, got {}",
            messages.len()
        );
        // All messages should be in the same phase.
        for msg in &messages {
            assert_eq!(msg.phase, LifecyclePhase::Onboarding);
        }
    }

    #[test]
    fn phase_transitions_after_duration() {
        let persona = test_persona(); // onboarding.duration_days = 3
        let mut runner = PersonaRunner::new(persona);

        assert_eq!(runner.current_phase(), LifecyclePhase::Onboarding);

        // Run through 3 onboarding days.
        for i in 0..3 {
            let date = base_date() + Duration::days(i);
            runner.generate_day(date);
        }

        // After 3 days, the next generate_day call should transition.
        let date = base_date() + Duration::days(3);
        let msgs = runner.generate_day(date);
        assert_eq!(runner.current_phase(), LifecyclePhase::Routine);
        for msg in &msgs {
            assert_eq!(msg.phase, LifecyclePhase::Routine);
        }
    }

    #[test]
    fn deterministic_with_same_seed() {
        let p1 = test_persona();
        let p2 = test_persona();
        let mut r1 = PersonaRunner::new(p1);
        let mut r2 = PersonaRunner::new(p2);

        let msgs1 = r1.generate_day(base_date());
        let msgs2 = r2.generate_day(base_date());

        assert_eq!(msgs1.len(), msgs2.len());
        for (a, b) in msgs1.iter().zip(msgs2.iter()) {
            assert_eq!(a.content, b.content, "deterministic content mismatch");
            assert_eq!(a.topic, b.topic, "deterministic topic mismatch");
            assert_eq!(a.is_correction, b.is_correction);
        }
    }

    #[test]
    fn introduces_facts_during_onboarding() {
        // Use a persona with very high introduction rate to make the test reliable.
        let mut persona = test_persona();
        persona.phases.onboarding.new_fact_introduction_rate = 1.0;
        persona.phases.onboarding.correction_rate = 0.0; // disable corrections so all slots are available

        let mut runner = PersonaRunner::new(persona);

        let mut total_facts = 0;
        for i in 0..3 {
            let date = base_date() + Duration::days(i);
            let msgs = runner.generate_day(date);
            for msg in &msgs {
                if let Some(ref gt) = msg.ground_truth {
                    if gt.introduces_fact.is_some() {
                        total_facts += 1;
                    }
                }
            }
        }

        assert!(
            total_facts >= 1,
            "expected at least 1 fact introduced over 3 days, got {total_facts}"
        );
    }
}
