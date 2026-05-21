use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeNodeType {
    Section,
    Text,
    Table,
    Code,
    Task,
    ListItem,
}

impl TreeNodeType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Section => "section",
            Self::Text => "text",
            Self::Table => "table",
            Self::Code => "code",
            Self::Task => "task",
            Self::ListItem => "list_item",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "section" => Self::Section,
            "text" => Self::Text,
            "table" => Self::Table,
            "code" => Self::Code,
            "task" => Self::Task,
            "listitem" | "list_item" => Self::ListItem,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Note,
    Task,
    Skill,
    Productivity,
    Okr,
    Learning,
}

impl SourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Note => "note",
            Self::Task => "task",
            Self::Skill => "skill",
            Self::Productivity => "productivity",
            Self::Okr => "okr",
            Self::Learning => "learning",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "note" => Self::Note,
            "task" => Self::Task,
            "skill" => Self::Skill,
            "productivity" => Self::Productivity,
            "okr" => Self::Okr,
            "learning" => Self::Learning,
            _ => Self::Note,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_type_roundtrip_all_variants() {
        let variants = [
            SourceType::Note,
            SourceType::Task,
            SourceType::Skill,
            SourceType::Productivity,
            SourceType::Okr,
            SourceType::Learning,
        ];
        for variant in &variants {
            let s = variant.as_str();
            let parsed = SourceType::parse(s);
            assert_eq!(parsed.as_str(), s, "Roundtrip failed for variant: {s}");
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub node_type: TreeNodeType,
    pub content: String,
    pub title: Option<String>,
    pub level: u32,
    pub source_type: SourceType,
    pub source_id: String,
    pub position: u32,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredNode {
    pub node: TreeNode,
    pub graph_score: f64,
    pub text_score: f64,
    pub combined: f64,
}

#[derive(Debug, Clone)]
pub struct EntityResolutionConfig {
    pub top_k: usize,
    pub gradient_threshold: f64,
    pub min_similarity: f64,
    pub use_llm_disambiguation: bool,
}

impl Default for EntityResolutionConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            gradient_threshold: 0.6,
            min_similarity: 0.3,
            use_llm_disambiguation: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BookRetrievalConfig {
    pub max_nodes: usize,
    pub max_map_nodes: usize,
    pub operator_timeout_ms: u64,
    pub pagerank_damping: f64,
    pub pagerank_iterations: u32,
}

impl Default for BookRetrievalConfig {
    fn default() -> Self {
        Self {
            max_nodes: 50,
            max_map_nodes: 10,
            operator_timeout_ms: 600,
            pagerank_damping: 0.85,
            pagerank_iterations: 20,
        }
    }
}
