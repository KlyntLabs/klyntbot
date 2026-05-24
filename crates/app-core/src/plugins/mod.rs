pub mod activity_log;
pub mod agent_tools;
pub mod ai_pipeline;
pub mod bash_toolkit;
pub mod brain_voice;
pub mod briefing;
pub mod coaching;
pub mod cognitive;
pub mod focus;
pub mod insights;
pub mod language_learning;
pub mod launcher;
pub mod learning;
pub mod lifecycle;
pub mod mirror;
pub mod notifications;
pub mod notes;
pub mod productivity;
pub mod tasks;
pub mod temporal;
pub mod voice;

use crate::plugin::AppCorePlugin;

/// The canonical, ordered list of every plugin the app registers.
///
/// This is the single source of truth for plugin registration — both
/// `init::init_app` (via `FeatureHostBuilder::with_plugins`) and the
/// dependency-graph tests consume it, so they can never drift apart.
/// The `FeatureHost` topologically re-sorts by declared `dependencies()`,
/// so the order here is only a tie-breaker, not a correctness guarantee.
pub fn all_plugins() -> Vec<Box<dyn AppCorePlugin>> {
    vec![
        Box::new(activity_log::ActivityLogPlugin),
        Box::new(focus::FocusPlugin),
        Box::new(notes::NotesPlugin),
        Box::new(tasks::TasksPlugin),
        Box::new(language_learning::LanguageLearningPlugin),
        Box::new(learning::LearningPlugin),
        Box::new(insights::InsightsPlugin),
        Box::new(cognitive::CognitivePlugin),
        Box::new(agent_tools::AgentToolsPlugin),
        Box::new(productivity::ProductivityPlugin),
        Box::new(launcher::LauncherPlugin),
        Box::new(coaching::CoachingPlugin),
        Box::new(mirror::MirrorPlugin),
        Box::new(brain_voice::BrainVoicePlugin),
        Box::new(voice::VoicePlugin),
        Box::new(briefing::BriefingPlugin),
        Box::new(lifecycle::LifecyclePlugin),
        Box::new(notifications::NotificationPlugin),
        Box::new(bash_toolkit::BashToolkitPlugin),
        Box::new(temporal::TemporalPlugin),
        Box::new(ai_pipeline::AiPipelinePlugin),
    ]
}

#[cfg(test)]
mod tests {
    use super::all_plugins;
    use crate::plugin::toposort::resolve_order;
    use std::collections::HashSet;

    #[test]
    fn canonical_plugin_graph_resolves() {
        // The real registration list must be acyclic and every declared
        // dependency must name a registered plugin. This guards against a
        // plugin shipping a typo'd or dangling `dependencies()` entry.
        let resolved = resolve_order(all_plugins())
            .expect("real plugin dependency graph must resolve without cycles or missing deps");
        assert_eq!(resolved.len(), all_plugins().len());
    }

    #[test]
    fn plugin_names_are_unique() {
        let plugins = all_plugins();
        let mut seen = HashSet::new();
        for p in &plugins {
            assert!(
                seen.insert(p.name().to_string()),
                "duplicate plugin name: {}",
                p.name()
            );
        }
    }

    #[test]
    fn dependencies_resolve_before_dependents() {
        // Every plugin must appear after all plugins it declares a dependency on.
        let resolved = resolve_order(all_plugins()).unwrap();
        let order: Vec<&str> = resolved.iter().map(|p| p.name()).collect();
        let pos = |name: &str| order.iter().position(|n| *n == name).unwrap();
        for p in &resolved {
            for dep in p.dependencies() {
                assert!(
                    pos(dep) < pos(p.name()),
                    "dependency '{}' must come before dependent '{}' (order: {:?})",
                    dep,
                    p.name(),
                    order
                );
            }
        }
    }
}
