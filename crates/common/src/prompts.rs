//! Interactive prompt types for bidirectional agent-CLI communication.
//!
//! These types live in `common` (Layer 0) so both `tools` (Layer 3, emitter)
//! and `agent`/`cli` (Layers 5-6, handler) can use them without circular deps.

/// A request from a tool to show an interactive prompt to the user.
#[derive(Debug, Clone)]
pub struct PromptRequest {
    /// The type of interactive prompt to display.
    pub prompt_type: PromptType,
    /// Tool-specific context (e.g., the created todo item as JSON).
    pub context: serde_json::Value,
}

/// Types of interactive prompts that tools can request.
#[derive(Debug, Clone)]
pub enum PromptType {
    /// Single-select with arrow key navigation.
    Select {
        header: String,
        options: Vec<PromptOption>,
        default_idx: usize,
    },
    /// Single-select with optional TAB-expandable text input per option.
    SelectWithInput {
        header: String,
        options: Vec<PromptOptionWithInput>,
        default_idx: usize,
    },
    /// Multi-select with space-to-toggle.
    MultiSelect {
        header: String,
        options: Vec<PromptOption>,
    },
    /// Simple yes/no confirmation.
    YesNo { question: String, default: bool },
}

/// A selectable option for Select and MultiSelect prompts.
#[derive(Debug, Clone)]
pub struct PromptOption {
    pub label: String,
    pub description: String,
}

/// A selectable option with optional text input expansion.
#[derive(Debug, Clone)]
pub struct PromptOptionWithInput {
    pub label: String,
    pub description: String,
    pub expandable: bool,
    pub input_hint: Option<String>,
}

/// User's response to an interactive prompt.
#[derive(Debug, Clone)]
pub enum UserResponse {
    /// Selected a single option by index.
    Selected(usize),
    /// Selected an option with optional text input.
    SelectedWithInput { index: usize, text: Option<String> },
    /// Selected multiple options by indices.
    MultiSelected(Vec<usize>),
    /// Yes/no answer.
    YesNo(bool),
    /// User cancelled the prompt (Ctrl+C).
    Cancelled,
}
