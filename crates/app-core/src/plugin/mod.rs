pub mod context;
pub mod host;
pub mod toposort;

use async_trait::async_trait;
use context::PluginContext;
use tools_core::FeatureMigration;

/// A feature that participates in full application initialization.
///
/// This is the single lifecycle trait for app-core plugins. Feature crates should
/// expose an `AppCorePlugin` impl that wires their contributions (tools, context
/// sources, signal consumers, AI features, metrics, cron jobs, background tasks).
///
/// `tools_core::FeaturePackage` is a metadata trait for feature crates (migrations,
/// health, config). It is *not* a lifecycle trait — plugins delegate to it for
/// migrations but do not implement it directly.
///
/// `ai_core::AiFeature` (via `#[derive(AiFeature)]`) generates `register()` for the
/// AI feature registry. Plugins call `ctx.register_ai_feature(|reg| MyFeature::register(reg))`
/// during `init()`.
///
/// Plugins are **active**: they call `ctx.register_tool()`, `ctx.add_context_source()`,
/// etc., rather than returning passive data structures. This lets a plugin conditionally
/// register based on config without the host knowing the conditions.
#[async_trait]
pub trait AppCorePlugin: Send + Sync + 'static {
    /// Unique plugin name (used for logging and diagnostics).
    fn name(&self) -> &str;

    /// SQL migrations to run before `init()` is called.
    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![]
    }

    /// Plugin names that must be initialized before this plugin.
    ///
    /// The host performs a topological sort; if a dependency is missing or a
    /// cycle exists, initialization fails at build time.
    fn dependencies(&self) -> &[&str] {
        &[]
    }

    /// Active registration phase (before AppCore is assembled).
    ///
    /// The plugin uses `ctx` to register tools, context sources, signal consumers,
    /// event translators, cron handlers, and background tasks. It may also insert
    /// typed handles into `ctx.host` for later retrieval.
    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()>;

    /// Post-assembly phase (after AppCore is fully constructed).
    ///
    /// Use this for registrations that need AppCore itself, such as tools that
    /// call back into AppCore methods.
    async fn post_init(&self, _app: &crate::state::AppCore) -> common::Result<()> {
        Ok(())
    }
}
