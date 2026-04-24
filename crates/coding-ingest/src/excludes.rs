//! Path-based privacy filter applied at the hook level — matches get dropped
//! before they reach the socket or the cold-path buffer.

use crate::event::{AgentEvent, EventKind};
use globset::{Glob, GlobSet, GlobSetBuilder};

/// The default exclusion globs shipped with klyntbot.
#[must_use]
pub fn default_exclude_globs() -> Vec<String> {
    vec![
        "**/.env", "**/.env.*",
        "**/secrets/**", "**/private/**",
        "**/*.key", "**/*.pem", "**/*.p12", "**/*.pfx",
        "**/id_rsa", "**/id_ed25519", "**/known_hosts",
        "**/.aws/credentials", "**/.gcloud/**", "**/.kube/config",
        "**/node_modules/**", "**/target/**", "**/.git/**",
    ].into_iter().map(String::from).collect()
}

/// Compiled glob matcher.
#[derive(Debug, Clone)]
pub struct ExcludeSet {
    set: GlobSet,
}

impl ExcludeSet {
    /// Compile a set of glob patterns.
    pub fn compile(patterns: &[String]) -> Result<Self, globset::Error> {
        let mut b = GlobSetBuilder::new();
        for p in patterns { b.add(Glob::new(p)?); }
        Ok(Self { set: b.build()? })
    }

    /// Should this event be dropped?
    #[must_use]
    pub fn should_drop(&self, event: &AgentEvent) -> bool {
        let AgentEvent::V1(v1) = event;
        match &v1.kind {
            EventKind::FileEdit { path, .. } | EventKind::FileEditEnriched { path, .. } => {
                self.set.is_match(path)
            }
            EventKind::ToolCall { args_preview, .. } => self.any_token_match(args_preview),
            _ => false,
        }
    }

    fn any_token_match(&self, haystack: &str) -> bool {
        // Split on common separators; match any token that looks like a path.
        haystack.split(|c: char| c.is_whitespace() || c == '=' || c == ',' || c == '"')
            .any(|tok| !tok.is_empty() && self.set.is_match(tok))
    }
}
