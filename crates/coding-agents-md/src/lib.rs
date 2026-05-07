use std::fs;
use std::path::{Path, PathBuf};

/// `WorkspaceAgentsSource` — the canonical entry-point for walking and
/// bundling AGENTS.md instructions at coding-thread start.
///
/// Note: this is **not** a `context_engine::ContextSource` trait impl. Per
/// spec §7, the AGENTS.md bundle is built once at thread start and persisted
/// as a synthetic user message — it does NOT re-execute every turn. The
/// `ContextSource` trait is for per-turn relevance-filtered injection, which
/// would be the wrong abstraction here. Keeping this as a value-builder
/// preserves the spec semantics while giving the spec-mandated symbol a home.
pub struct WorkspaceAgentsSource {
    workspace_path: PathBuf,
    global_path: Option<PathBuf>,
}

impl WorkspaceAgentsSource {
    pub fn new(workspace_path: PathBuf) -> Self {
        Self {
            workspace_path,
            global_path: None,
        }
    }

    pub fn with_global(mut self, global_path: PathBuf) -> Self {
        self.global_path = Some(global_path);
        self
    }

    /// Walk + format. Returns the synthetic-user-message body, or `None`
    /// when no AGENTS.md files were found anywhere on the chain.
    pub fn build_bundle(&self) -> Option<String> {
        let mut sources = walk_agents_md(&self.workspace_path);
        if let Some(global) = &self.global_path {
            match fs::read_to_string(global) {
                Ok(contents) => sources.insert(
                    0,
                    AgentsMdSource {
                        path: global.clone(),
                        dir: PathBuf::from("<global>"),
                        contents,
                    },
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => tracing::warn!("read {}: {e}", global.display()),
            }
        }
        if sources.is_empty() {
            return None;
        }
        Some(format_agents_md_bundle(&sources))
    }

    pub fn walk(&self) -> Vec<AgentsMdSource> {
        walk_agents_md(&self.workspace_path)
    }
}

/// A discovered AGENTS.md file and its context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentsMdSource {
    /// Absolute path to the AGENTS.md file.
    pub path: PathBuf,
    /// Directory the file was found in.
    pub dir: PathBuf,
    /// File contents.
    pub contents: String,
}

/// Walk from `start` upward to the filesystem root, collecting every
/// `AGENTS.md` file found along the way. Results are ordered outermost-first
/// (root-most → start-most).
pub fn walk_agents_md(start: &Path) -> Vec<AgentsMdSource> {
    let mut sources = Vec::new();
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("AGENTS.md");
        if candidate.exists() {
            if let Ok(contents) = fs::read_to_string(&candidate) {
                sources.push(AgentsMdSource {
                    path: candidate,
                    dir: cur.clone(),
                    contents,
                });
            }
        }
        match cur.parent() {
            Some(parent) => cur = parent.to_path_buf(),
            None => break,
        }
    }
    sources.reverse();
    sources
}

/// Format discovered AGENTS.md sources into a bundle suitable for injection
/// into the agent's system prompt.
pub fn format_agents_md_bundle(sources: &[AgentsMdSource]) -> String {
    sources
        .iter()
        .map(|s| {
            format!(
                "# AGENTS.md instructions for {}\n\n<INSTRUCTIONS>\n{}\n</INSTRUCTIONS>",
                s.dir.display(),
                s.contents
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn walk_finds_ancestor_chain() {
        let td = TempDir::new().unwrap();
        let nested = td.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(td.path().join("AGENTS.md"), "root rules").unwrap();
        fs::write(td.path().join("a/AGENTS.md"), "a rules").unwrap();
        fs::write(nested.join("AGENTS.md"), "c rules").unwrap();

        let found = walk_agents_md(&nested);
        assert_eq!(found.len(), 3);
        // Outermost first
        assert_eq!(found[0].dir, td.path().to_path_buf());
        assert_eq!(found[2].dir, nested);
    }

    #[test]
    fn walk_returns_empty_when_no_agents_md() {
        let td = TempDir::new().unwrap();
        let found = walk_agents_md(td.path());
        assert!(found.is_empty());
    }

    #[test]
    fn format_bundle_uses_instruction_wrapper() {
        let td = TempDir::new().unwrap();
        fs::write(td.path().join("AGENTS.md"), "rule one").unwrap();
        let found = walk_agents_md(td.path());
        let bundle = format_agents_md_bundle(&found);
        assert!(bundle.contains("# AGENTS.md instructions for"));
        assert!(bundle.contains("<INSTRUCTIONS>"));
        assert!(bundle.contains("rule one"));
        assert!(bundle.contains("</INSTRUCTIONS>"));
    }

    #[test]
    fn walk_skips_missing_intermediate_dirs() {
        let td = TempDir::new().unwrap();
        fs::write(td.path().join("AGENTS.md"), "root only").unwrap();
        // No AGENTS.md in subdirectory
        let nested = td.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        let found = walk_agents_md(&nested);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].contents, "root only");
    }
}
