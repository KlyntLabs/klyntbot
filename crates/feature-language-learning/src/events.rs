use ai_core_macros::AiEvent;
use bus::DomainEvent;
use bus::LanguageLearningEvent as BusLanguageLearningEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "LanguageLearning")]
pub enum LanguageLearningEvent {
    #[ai(
        importance = 0.6,
        salience = "extract_if(*overall_score < 0.6 || !weak_phonemes.is_empty())",
        observation_template = "Pronunciation scored {overall_score} (session {session_id})",
        metric(
            name = "pronunciation_accuracy",
            value_from = *overall_score,
            window = "30d",
            min_samples = 5,
            aggregation = "avg",
        )
    )]
    PronunciationScored {
        session_id: String,
        overall_score: f64,
        weak_phonemes: Vec<String>,
    },

    #[ai(
        importance = 0.7,
        salience = "extract",
        observation_template = "Exam {exam_id}: {score} ({passed})",
        metric(
            name = "exam_score_trend",
            value_from = *score as f64,
            window = "90d",
            min_samples = 2,
            aggregation = "avg",
        )
    )]
    ExamAttempted {
        exam_id: String,
        score: u32,
        passed: bool,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Phoneme mastery: /{phoneme}/ at {mastery_level}"
    )]
    PhoneticMasteryGained { phoneme: String, mastery_level: f64 },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Practice session ({language}) done: {success_rate} success",
        metric(
            name = "language_practice_success_rate",
            value_from = *success_rate,
            window = "30d",
            min_samples = 5,
            aggregation = "avg",
        )
    )]
    PracticeSessionCompleted {
        session_id: String,
        language: String,
        duration_secs: u64,
        success_rate: f64,
    },
}

impl From<LanguageLearningEvent> for DomainEvent {
    fn from(e: LanguageLearningEvent) -> Self {
        match e {
            LanguageLearningEvent::PronunciationScored {
                session_id,
                overall_score,
                weak_phonemes,
            } => DomainEvent::LanguageLearning(BusLanguageLearningEvent::PronunciationScored {
                session_id,
                overall_score,
                weak_phonemes,
            }),
            LanguageLearningEvent::ExamAttempted {
                exam_id,
                score,
                passed,
            } => DomainEvent::LanguageLearning(BusLanguageLearningEvent::ExamAttempted {
                exam_id,
                score,
                passed,
            }),
            LanguageLearningEvent::PhoneticMasteryGained {
                phoneme,
                mastery_level,
            } => DomainEvent::LanguageLearning(BusLanguageLearningEvent::PhoneticMasteryGained {
                phoneme,
                mastery_level,
            }),
            LanguageLearningEvent::PracticeSessionCompleted {
                session_id,
                language,
                duration_secs,
                success_rate,
            } => DomainEvent::LanguageLearning(
                BusLanguageLearningEvent::LanguagePracticeSessionCompleted {
                    session_id,
                    language,
                    duration_secs,
                    success_rate,
                },
            ),
        }
    }
}

/// Translate a bus::DomainEvent into the typed LanguageLearningEvent, if applicable.
pub fn try_from_domain_event(e: &bus::DomainEvent) -> Option<LanguageLearningEvent> {
    use bus::DomainEvent;
    match e {
        DomainEvent::LanguageLearning(BusLanguageLearningEvent::PronunciationScored {
            session_id,
            overall_score,
            weak_phonemes,
        }) => Some(LanguageLearningEvent::PronunciationScored {
            session_id: session_id.clone(),
            overall_score: *overall_score,
            weak_phonemes: weak_phonemes.clone(),
        }),
        DomainEvent::LanguageLearning(BusLanguageLearningEvent::ExamAttempted {
            exam_id,
            score,
            passed,
        }) => Some(LanguageLearningEvent::ExamAttempted {
            exam_id: exam_id.clone(),
            score: *score,
            passed: *passed,
        }),
        DomainEvent::LanguageLearning(BusLanguageLearningEvent::PhoneticMasteryGained {
            phoneme,
            mastery_level,
        }) => Some(LanguageLearningEvent::PhoneticMasteryGained {
            phoneme: phoneme.clone(),
            mastery_level: *mastery_level,
        }),
        DomainEvent::LanguageLearning(
            BusLanguageLearningEvent::LanguagePracticeSessionCompleted {
                session_id,
                language,
                duration_secs,
                success_rate,
            },
        ) => Some(LanguageLearningEvent::PracticeSessionCompleted {
            session_id: session_id.clone(),
            language: language.clone(),
            duration_secs: *duration_secs,
            success_rate: *success_rate,
        }),
        _ => None,
    }
}
