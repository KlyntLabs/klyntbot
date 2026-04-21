/// Workspace-global enumeration of feature domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecallDomain {
    General,
    Tasks,
    Finance,
}

impl RecallDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecallDomain::General => "general",
            RecallDomain::Tasks   => "tasks",
            RecallDomain::Finance => "finance",
        }
    }
}

impl std::fmt::Display for RecallDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
