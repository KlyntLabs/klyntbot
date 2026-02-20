//! Tests for Todo types with DomainEnum (AC 4.2).
//!
//! Verifies that TodoStatus and CreationMode correctly use the DomainEnum
//! derive macro for from_str_loose, as_str, Display, and FromStr.
//!
//! Dev: implement these tests via TDD alongside the types in
//! `crates/feature-todo/src/types.rs` and `crates/feature-todo/src/config.rs`.

// NOTE: Adjust imports once feature-todo types are finalized.
//
// Expected imports:
//   use feature_todo::TodoStatus;
//   use feature_todo::config::CreationMode;  // or re-exported from lib.rs

// ============================================================
// AC 4.2: TodoStatus::from_str_loose with aliases
// ============================================================

mod todo_status {
    // use feature_todo::TodoStatus;

    #[test]
    fn test_todo_status_from_str_loose_pending() {
        // Verifies: TodoStatus::from_str_loose("pending") == Some(TodoStatus::Todo).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_open() {
        // Verifies: TodoStatus::from_str_loose("open") == Some(TodoStatus::Todo).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_in_progress() {
        // Verifies: TodoStatus::from_str_loose("in_progress") == Some(TodoStatus::Doing).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_active() {
        // Verifies: TodoStatus::from_str_loose("active") == Some(TodoStatus::Doing).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_completed() {
        // Verifies: TodoStatus::from_str_loose("completed") == Some(TodoStatus::Done).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_closed() {
        // Verifies: TodoStatus::from_str_loose("closed") == Some(TodoStatus::Done).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_case_insensitive() {
        // Verifies: TodoStatus::from_str_loose("TODO") == Some(TodoStatus::Todo),
        //           TodoStatus::from_str_loose("DOING") == Some(TodoStatus::Doing).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_loose_unknown() {
        // Verifies: TodoStatus::from_str_loose("invalid") == None.
        // AC 4.2
        todo!()
    }

    // ============================================================
    // AC 4.2: TodoStatus::as_str returns snake_case
    // ============================================================

    #[test]
    fn test_todo_status_as_str_todo() {
        // Verifies: TodoStatus::Todo.as_str() == "todo".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_as_str_doing() {
        // Verifies: TodoStatus::Doing.as_str() == "doing".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_as_str_done() {
        // Verifies: TodoStatus::Done.as_str() == "done".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_as_str_archived() {
        // Verifies: TodoStatus::Archived.as_str() == "archived".
        // AC 4.2
        todo!()
    }

    // ============================================================
    // AC 4.2: TodoStatus Display and FromStr
    // ============================================================

    #[test]
    fn test_todo_status_display() {
        // Verifies: format!("{}", TodoStatus::Todo) == "todo".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_todo_status_from_str_roundtrip() {
        // Verifies: "todo".parse::<TodoStatus>() == Ok(TodoStatus::Todo),
        //           then TodoStatus::Todo.to_string() == "todo".
        // AC 4.2
        todo!()
    }
}

// ============================================================
// AC 4.2: CreationMode with DomainEnum
// ============================================================

mod creation_mode {
    // use feature_todo::config::CreationMode;

    #[test]
    fn test_creation_mode_as_str_ask_first() {
        // Verifies: CreationMode::AskFirst.as_str() == "ask_first".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_as_str_yolo() {
        // Verifies: CreationMode::Yolo.as_str() == "yolo".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_as_str_party() {
        // Verifies: CreationMode::Party.as_str() == "party".
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_from_str_loose_alias() {
        // Verifies: CreationMode::from_str_loose("ask_first") == Some(CreationMode::AskFirst).
        // The alias "ask_first" should map to AskFirst.
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_from_str_loose_case_insensitive() {
        // Verifies: CreationMode::from_str_loose("YOLO") == Some(CreationMode::Yolo).
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_from_str_loose_unknown() {
        // Verifies: CreationMode::from_str_loose("invalid") == None.
        // AC 4.2
        todo!()
    }

    #[test]
    fn test_creation_mode_display() {
        // Verifies: format!("{}", CreationMode::AskFirst) == "ask_first".
        // AC 4.2
        todo!()
    }
}
