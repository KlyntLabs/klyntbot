use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin that registers the bash tool kit (edit, bash, str_replace, etc.)
/// and wires hook engines into the agent runtime.
pub struct BashToolkitPlugin;

#[async_trait]
impl AppCorePlugin for BashToolkitPlugin {
    fn name(&self) -> &str {
        "bash-toolkit"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.add_event_translator(crate::init::ai_pipeline::translate_bash_job);

        let (non_ui_policy, hooks_path) = {
            let config_guard = ctx.deps.config.read().await;
            let non_ui_policy = config_guard.tools.approval_policy.non_ui_channels;
            let hooks_path = config_guard.data_dir_path().join("hooks.toml");
            (non_ui_policy, hooks_path)
        };

        let exclude_globs: Vec<&str> = vec![];
        let privacy = Arc::new(
            klynt_core::privacy::PrivacyGuard::from_globs(&exclude_globs).expect("privacy globs"),
        );
        let policy = Arc::new(
            dirs::home_dir()
                .map(|h| h.join(".klyntbot/rules"))
                .and_then(|p| klynt_execpolicy::Policy::load_from_dir(&p).ok())
                .unwrap_or_else(klynt_execpolicy::Policy::empty),
        );
        let bus = ctx
            .deps
            .domain_event_bus
            .clone()
            .unwrap_or_else(|| Arc::new(bus::DomainEventBus::new(64)));

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

        let hook_engine: Option<Arc<klynt_hooks::HookEngine>> = {
            if hooks_path.exists() {
                match klynt_hooks::HookEngine::load_from_path(&hooks_path) {
                    Ok(e) => Some(Arc::new(e)),
                    Err(err) => {
                        tracing::warn!("klynt-hooks: failed to load {hooks_path:?}: {err}");
                        Some(Arc::new(klynt_hooks::HookEngine::empty()))
                    }
                }
            } else {
                Some(Arc::new(klynt_hooks::HookEngine::empty()))
            }
        };

        let kit = klynt_core::ToolKitBuilder {
            cwd: cwd.clone(),
            policy: policy.clone(),
            privacy: privacy.clone(),
            bus: bus.clone(),
            repos: ctx.deps.repos.clone(),
            non_ui_policy,
            hook_engine: hook_engine.clone(),
            snapshot_repo: None,
            session_key: String::new(),
            mirror_learning_enabled: false,
            mirror_min_approvals: 0,
            mirror_cooldown_seconds: 0,
            repo_id: String::new(),
        };

        // Register all bash toolkit tools into the plugin-built registry
        kit.register_all(ctx.tools);
        tracing::info!("Tool kit registered");

        // Store the kit builder + hook engine in FeatureHost so the agent phase
        // (init/mod.rs Phase 5) can inject them into the AgentLoop at construction.
        ctx.insert_handle(Arc::new(kit));
        if let Some(ref engine) = hook_engine {
            ctx.insert_handle(Arc::clone(engine));
        }

        Ok(())
    }
}
