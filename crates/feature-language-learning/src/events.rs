use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "LanguageLearning")]
pub enum LanguageLearningEvent {
    #[ai(
        importance = 0.6,
        salience = "extract_if(*overall_score < 0.6 || !weak_phonemes.is_empty())",
        observation_template = "Pronunciation scored {overall_score} (session {session_id})"
    )]
    PronunciationScored {
        session_id: String,
        overall_score: f64,
        weak_phonemes: Vec<String>,
    },

    #[ai(
        importance = 0.7,
        salience = "extract",
        observation_template = "Exam {exam_id}: {score} ({passed})"
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
        observation_template = "Practice session ({language}) done: {success_rate} success"
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
            } => DomainEvent::PronunciationScored {
                session_id,
                overall_score,
                weak_phonemes,
            },
            LanguageLearningEvent::ExamAttempted {
                exam_id,
                score,
                passed,
            } => DomainEvent::ExamAttempted {
                exam_id,
                score,
                passed,
            },
            LanguageLearningEvent::PhoneticMasteryGained {
                phoneme,
                mastery_level,
            } => DomainEvent::PhoneticMasteryGained {
                phoneme,
                mastery_level,
            },
            LanguageLearningEvent::PracticeSessionCompleted {
                session_id,
                language,
                duration_secs,
                success_rate,
            } => DomainEvent::LanguagePracticeSessionCompleted {
                session_id,
                language,
                duration_secs,
                success_rate,
            },
        }
    }
}
