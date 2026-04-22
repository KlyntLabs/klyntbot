//! MetaRuleSignalSource — proposes meta-rules from correction streaks and
//! low-confidence routing patterns. Replaces MetaRuleDetector.

use std::collections::HashMap;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;
use tracing::warn;
use uuid::Uuid;

use crate::mirror::{
    snippet_from_alert, MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorAlert,
    MirrorRepo,
};

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.4;
const SAME_SESSION_CORRECTION_THRESHOLD: u32 = 2;
const CROSS_SESSION_CORRECTION_THRESHOLD: u32 = 3;
const LOW_CONFIDENCE_STREAK_THRESHOLD: u32 = 3;
const MAX_TRACKED_SESSIONS: usize = 100;
const MAX_TRACKED_SKILLS: usize = 50;

pub struct MetaRuleSignalSource {
    session_corrections: std::sync::Mutex<HashMap<String, u32>>,
    skill_corrections: std::sync::Mutex<HashMap<String, u32>>,
    low_confidence_streak: std::sync::atomic::AtomicU32,
    repo: MirrorRepo,
}

impl MetaRuleSignalSource {
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            session_corrections: std::sync::Mutex::new(HashMap::new()),
            skill_corrections: std::sync::Mutex::new(HashMap::new()),
            low_confidence_streak: std::sync::atomic::AtomicU32::new(0),
            repo,
        }
    }

    fn record_correction(
        &self,
        session_key: &str,
        skill_name: &str,
    ) -> Option<MirrorAlert> {
        let mut sessions = self.session_corrections.lock().unwrap();
        if sessions.len() > MAX_TRACKED_SESSIONS {
            sessions.clear();
        }
        let session_count = {
            let c = sessions.entry(session_key.to_string()).or_insert(0);
            *c += 1;
            *c
        };

        if session_count >= SAME_SESSION_CORRECTION_THRESHOLD {
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "User corrected me {session_count} times in the same session \
                     — ask for clarification before responding"
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        drop(sessions);

        let mut skills = self.skill_corrections.lock().unwrap();
        if skills.len() > MAX_TRACKED_SKILLS {
            skills.clear();
        }
        let skill_count = {
            let c = skills.entry(skill_name.to_string()).or_insert(0);
            *c += 1;
            *c
        };

        if skill_count >= CROSS_SESSION_CORRECTION_THRESHOLD {
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "User has corrected me {skill_count} times across sessions for the \
                     '{skill_name}' skill — adjust routing or response strategy"
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        None
    }

    fn record_low_confidence(&self,
        skill: &str,
        confidence: f64,
    ) -> Option<MirrorAlert> {
        if confidence >= LOW_CONFIDENCE_THRESHOLD {
            return None;
        }

        let streak = self.low_confidence_streak.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

        if streak >= LOW_CONFIDENCE_STREAK_THRESHOLD {
            self.low_confidence_streak.store(0, std::sync::atomic::Ordering::Relaxed);
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "Routing confidence has been consistently low (latest: {confidence:.2} \
                     for '{skill}') — consider adding trigger keywords or adjusting skill scopes"
                ),
                source: MetaRuleSource::ReflectionGenerated,
            });
        }

        None
    }

    fn record_high_confidence(&self) {
        self.low_confidence_streak.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    async fn handle_alert(&self, alert: &MirrorAlert) {
        if let MirrorAlert::MetaRuleProposed {
            rule_id,
            rule_text,
            source,
        } = alert
        {
            let rule = MetaRule {
                id: *rule_id,
                trigger_condition: rule_text.clone(),
                action: MetaRuleAction::ForceClarification,
                source: source.clone(),
                effectiveness_score: 0.5,
                status: MetaRuleStatus::Pending,
                signal_count: 1,
                created_at: Timestamp::now(),
                updated_at: Timestamp::now(),
            };

            if let Err(e) = self.repo.insert_meta_rule(&rule).await {
                warn!("MetaRuleSignalSource: failed to insert meta-rule: {e}");
            }

            let snippet = snippet_from_alert(alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("MetaRuleSignalSource: failed to insert snippet: {e}");
            }
        }
    }
}

#[async_trait]
impl MirrorSignalSource for MetaRuleSignalSource {
    const SPEC: MirrorSnapshotSpec = MirrorSnapshotSpec {
        name: "meta_rule",
        subscribed_kinds: &["UserCorrectedAI", "SkillRouted"],
        flush_interval_secs: None,
    };

    fn name(&self) -> &'static str {
        "meta-rule-signal-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(raw) = &signal.raw_event {
            match raw {
                bus::DomainEvent::UserCorrectedAI {
                    session_key,
                    active_skill,
                    ..
                } => {
                    let skill = active_skill.as_deref().unwrap_or("unknown");
                    if let Some(alert) = self.record_correction(session_key, skill) {
                        self.handle_alert(&alert).await;
                    }
                }
                bus::DomainEvent::SkillRouted {
                    confidence,
                    skill_name,
                    ..
                } => {
                    if *confidence < LOW_CONFIDENCE_THRESHOLD {
                        if let Some(alert) = self.record_low_confidence(skill_name, *confidence) {
                            self.handle_alert(&alert).await;
                        }
                    } else {
                        self.record_high_confidence();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Event-driven only — no periodic flush.
        Ok(())
    }
}
