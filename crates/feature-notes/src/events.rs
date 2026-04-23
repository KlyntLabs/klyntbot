use ai_core_macros::AiEvent;
use bus::DomainEvent;

#[derive(Debug, Clone, AiEvent)]
#[ai(domain = "Notes")]
pub enum NoteEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Created note '{title}'",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    Created {
        note_id: String,
        title: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Updated note '{title}'",
        entity_bridge(type = "note", name_from = "title", id_from = "note_id"),
    )]
    Updated {
        note_id: String,
        title: String,
    },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Note content changed",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    ContentChanged {
        note_id: String,
    },

    #[ai(
        importance = 0.4,
        salience = "extract",
        observation_template = "Finished editing note",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    EditingFinished {
        note_id: String,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Deleted note",
    )]
    Deleted {
        note_id: String,
    },

    #[ai(
        importance = 0.5,
        salience = "extract",
        observation_template = "Studied note for {duration_secs}s",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    Studied {
        note_id: String,
        duration_secs: u64,
        atoms_reviewed: usize,
        mode: String,
    },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Practice unit completed for note {note_id}",
        entity_bridge(type = "note", name_from = "note_id", id_from = "note_id"),
    )]
    PracticeUnitCompleted {
        session_id: String,
        note_id: String,
        unit_index: u32,
        grade: String,
        scores: String,
        confidence_rating: u8,
        edited: bool,
    },

    #[ai(
        importance = 0.6,
        salience = "extract",
        observation_template = "Practice session done: {average_score:.1}",
    )]
    PracticeSessionCompleted {
        session_id: String,
        note_id: String,
        units_completed: u32,
        average_score: f64,
        source_lang: String,
        target_lang: String,
        weak_unit_count: u32,
    },

    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Translation completed for note {note_id}",
    )]
    TranslationCompleted {
        note_id: String,
        source_lang: String,
        target_lang: String,
        word_count: usize,
        is_selection: bool,
    },
}

impl From<NoteEvent> for DomainEvent {
    fn from(e: NoteEvent) -> Self {
        match e {
            NoteEvent::Created { note_id, title } => DomainEvent::NoteCreated { note_id, title },
            NoteEvent::Updated { note_id, title } => DomainEvent::NoteUpdated { note_id, title },
            NoteEvent::ContentChanged { note_id } => DomainEvent::NoteContentChanged { note_id },
            NoteEvent::EditingFinished { note_id } => DomainEvent::NoteEditingFinished { note_id },
            NoteEvent::Deleted { note_id } => DomainEvent::NoteDeleted { note_id },
            NoteEvent::Studied {
                note_id,
                duration_secs,
                atoms_reviewed,
                mode,
            } => DomainEvent::NoteStudied {
                note_id,
                duration_secs,
                atoms_reviewed,
                mode,
            },
            NoteEvent::PracticeUnitCompleted {
                session_id,
                note_id,
                unit_index,
                grade,
                scores,
                confidence_rating,
                edited,
            } => DomainEvent::PracticeUnitCompleted {
                session_id,
                note_id,
                unit_index,
                grade,
                scores,
                confidence_rating,
                edited,
            },
            NoteEvent::PracticeSessionCompleted {
                session_id,
                note_id,
                units_completed,
                average_score,
                source_lang,
                target_lang,
                weak_unit_count,
            } => DomainEvent::PracticeSessionCompleted {
                session_id,
                note_id,
                units_completed,
                average_score,
                source_lang,
                target_lang,
                weak_unit_count,
            },
            NoteEvent::TranslationCompleted {
                note_id,
                source_lang,
                target_lang,
                word_count,
                is_selection,
            } => DomainEvent::TranslationCompleted {
                note_id,
                source_lang,
                target_lang,
                word_count,
                is_selection,
            },
        }
    }
}
