//! TracingRegistry — provider-id → provider dispatch.

use std::collections::HashMap;
use std::sync::Arc;

use common::{KlyntbotError, Result};

use super::provider::TracingProvider;
use super::types::ProviderInfo;

/// Holds all registered `TracingProvider`s by id.
#[derive(Default)]
pub struct TracingRegistry {
    providers: HashMap<&'static str, Arc<dyn TracingProvider>>,
}

impl TracingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Arc<dyn TracingProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn TracingProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| KlyntbotError::StorageNotFound(format!("tracing provider: {id}")))
    }

    /// Cheap snapshot for the FE provider chip row. Calls each provider's
    /// `list_sessions` so the count badge is accurate at render time.
    pub async fn list_providers(&self) -> Result<Vec<ProviderInfo>> {
        let mut out = Vec::with_capacity(self.providers.len());
        for (id, p) in &self.providers {
            let sessions = p.list_sessions().await.unwrap_or_default();
            out.push(ProviderInfo {
                id: (*id).to_string(),
                display_name: p.display_name().to_string(),
                session_count: sessions.len() as u32,
            });
        }
        out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::types::*;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};

    struct Stub;
    #[async_trait]
    impl TracingProvider for Stub {
        fn id(&self) -> &'static str {
            "stub"
        }
        fn display_name(&self) -> &'static str {
            "Stub"
        }
        async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
            Ok(vec![])
        }
        async fn load_session(&self, _: &str, _: Scope) -> Result<SessionDetail> {
            unimplemented!()
        }
        async fn load_context(&self, _: &str, _: Scope) -> Result<Vec<ContextMessage>> {
            Ok(vec![])
        }
        async fn load_state(&self, _: &str) -> Result<SessionState> {
            Ok(SessionState::default())
        }
        async fn list_subagents(&self, _: &str) -> Result<Vec<SubagentSummary>> {
            Ok(vec![])
        }
        async fn import_from_file(&self, _: &Path) -> Result<String> {
            Ok("x".into())
        }
        async fn open_dir(&self, _: &str) -> Result<PathBuf> {
            Ok(PathBuf::from("/"))
        }
        async fn stats(&self) -> Result<StatsBundle> {
            Ok(StatsBundle::default())
        }
        async fn session_summary(&self, _: &str) -> Result<SessionSummary> {
            unimplemented!()
        }
        async fn load_subagent_session(&self, _: &str, _: &str) -> Result<SessionDetail> {
            unimplemented!()
        }
        async fn load_subagent_context(&self, _: &str, _: &str) -> Result<Vec<ContextMessage>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn default_supported_tabs_match_kimi_legacy() {
        let stub = Stub;
        let tabs = stub.supported_tabs();
        assert_eq!(
            tabs,
            &[
                SessionTab::Wire,
                SessionTab::Context,
                SessionTab::State,
                SessionTab::Dual,
                SessionTab::Agents,
            ]
        );
    }

    #[tokio::test]
    async fn default_header_layout_matches_kimi_legacy() {
        let stub = Stub;
        let chips = stub.header_layout();
        assert!(chips.contains(&HeaderChip::Turns));
        assert!(chips.contains(&HeaderChip::Steps));
        assert!(chips.contains(&HeaderChip::Compactions));
    }

    #[tokio::test]
    async fn register_and_get() {
        let mut r = TracingRegistry::new();
        r.register(Arc::new(Stub));
        assert_eq!(r.get("stub").unwrap().id(), "stub");
        assert!(r.get("missing").is_err());
    }

    #[tokio::test]
    async fn list_providers_returns_all() {
        let mut r = TracingRegistry::new();
        r.register(Arc::new(Stub));
        let infos = r.list_providers().await.unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, "stub");
    }
}
