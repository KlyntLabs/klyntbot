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
            Self::Section => "Section",
            Self::Text => "Text",
            Self::Table => "Table",
            Self::Code => "Code",
            Self::Task => "Task",
            Self::ListItem => "ListItem",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "Section" => Self::Section,
            "Text" => Self::Text,
            "Table" => Self::Table,
            "Code" => Self::Code,
            "Task" => Self::Task,
            "ListItem" => Self::ListItem,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Note,
    Task,
    Skill,
}

impl SourceType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Note => "Note",
            Self::Task => "Task",
            Self::Skill => "Skill",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "Note" => Self::Note,
            "Task" => Self::Task,
            "Skill" => Self::Skill,
            _ => Self::Note,
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
