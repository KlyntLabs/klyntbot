/// Workspace-global enumeration of feature domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallDomain {
    General,
    Tasks,
    Finance,
    Productivity,
    Learning,
    Mirror,
    Coaching,
    Notes,
    LanguageLearning,
}

impl RecallDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecallDomain::General => "general",
            RecallDomain::Tasks => "tasks",
            RecallDomain::Finance => "finance",
            RecallDomain::Productivity => "productivity",
            RecallDomain::Learning => "learning",
            RecallDomain::Mirror => "mirror",
            RecallDomain::Coaching => "coaching",
            RecallDomain::Notes => "notes",
            RecallDomain::LanguageLearning => "language_learning",
        }
    }

    /// Parse a domain string into a `RecallDomain`. Unknown values fall back
    /// to `General` so callers can pass through `DomainEvent.domain: String`
    /// without a per-site match.
    pub fn from_str_or_general(s: &str) -> Self {
        match s {
            "tasks" => RecallDomain::Tasks,
            "finance" => RecallDomain::Finance,
            "productivity" => RecallDomain::Productivity,
            "learning" => RecallDomain::Learning,
            "mirror" => RecallDomain::Mirror,
            "coaching" => RecallDomain::Coaching,
            "notes" => RecallDomain::Notes,
            "language_learning" => RecallDomain::LanguageLearning,
            _ => RecallDomain::General,
        }
    }
}

impl std::fmt::Display for RecallDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
