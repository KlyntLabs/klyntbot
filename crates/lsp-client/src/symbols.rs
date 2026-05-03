//! Document symbol queries for anchored_symbols.

use serde::{Deserialize, Serialize};

/// A symbol anchored to a specific line range in a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchoredSymbol {
    /// Symbol name (e.g. function name, struct name).
    pub name: String,
    /// Symbol kind (e.g. "function", "struct", "method").
    pub kind: SymbolKind,
    /// 1-indexed start line.
    pub start_line: u32,
    /// 1-indexed end line.
    pub end_line: u32,
    /// Optional detail string (e.g. function signature).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Module,
    Constant,
    TypeAlias,
    Variable,
    Other,
}

/// Convert an `lsp_types::DocumentSymbol` to our `AnchoredSymbol`.
impl From<&lsp_types::DocumentSymbol> for AnchoredSymbol {
    fn from(s: &lsp_types::DocumentSymbol) -> Self {
        let kind = match s.kind {
            lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
            lsp_types::SymbolKind::METHOD => SymbolKind::Method,
            lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
            lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
            lsp_types::SymbolKind::INTERFACE => SymbolKind::Trait,
            lsp_types::SymbolKind::MODULE => SymbolKind::Module,
            lsp_types::SymbolKind::CONSTANT => SymbolKind::Constant,
            lsp_types::SymbolKind::TYPE_PARAMETER => SymbolKind::TypeAlias,
            lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
            _ => SymbolKind::Other,
        };
        Self {
            name: s.name.clone(),
            kind,
            start_line: s.range.start.line + 1,
            end_line: s.range.end.line + 1,
            detail: s.detail.clone(),
        }
    }
}
