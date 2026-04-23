use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Learning")]
pub enum LearningEvent {
    #[ai(
        importance = 0.6,
        salience = "extract",
        observation_template = "Knowledge atom extracted from note {note_id}: {text}",
        entity_bridge(type = "knowledge_atom", name_from = "text", id_from = "atom_id"),
    )]
    AtomExtracted {
        atom_id: String,
        note_id: String,
        text: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Knowledge atom created: {atom_id}",
    )]
    AtomCreated {
        atom_id: String,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Knowledge atom accepted: {atom_id}",
    )]
    AtomAccepted {
        atom_id: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Knowledge atom archived: {atom_id}",
    )]
    AtomArchived {
        atom_id: String,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Flashcard reviewed (quality {quality}): {card_id}",
    )]
    FlashcardReviewed {
        atom_id: String,
        card_id: String,
        quality: u8,
        recall_speed_ms: u64,
        new_retention_pct: f64,
        source_note_id: Option<String>,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Atom reinforced: {atom_id}",
    )]
    AtomReinforced {
        atom_id: String,
    },

    #[ai(
        importance = 0.4,
        salience = "extract",
        observation_template = "Flashcard scheduled: {card_id} due at {due_at}",
        entity_bridge(type = "knowledge_atom", name_from = "atom_id", id_from = "atom_id"),
    )]
    FlashcardScheduled {
        card_id: String,
        atom_id: String,
        due_at: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Atom retention decayed to {retention}: {atom_id}",
    )]
    RetentionDecayed {
        atom_id: String,
        retention: f64,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Atom linked to semantic fact: {atom_id} -> {fact_id} ({similarity})",
    )]
    SemanticFactLinked {
        atom_id: String,
        fact_id: String,
        similarity: f64,
    },
}

impl From<LearningEvent> for DomainEvent {
    fn from(e: LearningEvent) -> Self {
        match e {
            LearningEvent::AtomExtracted {
                atom_id,
                note_id,
                text,
            } => DomainEvent::KnowledgeAtomExtracted {
                atom_id,
                note_id,
                text,
            },
            LearningEvent::AtomCreated { atom_id } => DomainEvent::KnowledgeAtomCreated {
                atom_id,
                atom_type: String::new(),
                domain: String::new(),
                source_note_id: None,
                personal_importance: 0.0,
            },
            LearningEvent::AtomAccepted { atom_id } => DomainEvent::KnowledgeAtomAccepted {
                atom_id,
                atom_type: String::new(),
            },
            LearningEvent::AtomArchived { atom_id } => DomainEvent::KnowledgeAtomArchived {
                atom_id,
                reason: String::new(),
            },
            LearningEvent::FlashcardReviewed {
                atom_id,
                card_id,
                quality,
                recall_speed_ms,
                new_retention_pct,
                source_note_id,
            } => DomainEvent::AtomFlashcardReviewed {
                atom_id,
                card_id,
                quality,
                recall_speed_ms,
                new_retention_pct,
                source_note_id,
            },
            LearningEvent::AtomReinforced { atom_id } => DomainEvent::AtomReinforced {
                atom_id,
                referencing_note_id: String::new(),
                new_salience: 0.0,
                subject: String::new(),
                domain: String::new(),
                reinforcement_count: 0,
            },
            LearningEvent::FlashcardScheduled {
                card_id,
                atom_id,
                due_at,
            } => DomainEvent::FlashcardScheduled {
                flashcard_id: card_id,
                atom_id,
                due_at,
            },
            LearningEvent::RetentionDecayed { atom_id, retention } => {
                DomainEvent::AtomRetentionDecayed { atom_id, retention }
            }
            LearningEvent::SemanticFactLinked {
                atom_id,
                fact_id,
                similarity,
            } => DomainEvent::AtomSemanticFactLinked {
                atom_id,
                fact_id,
                similarity,
            },
        }
    }
}
