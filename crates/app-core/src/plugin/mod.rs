pub mod context;
pub mod host;

use async_trait::async_trait;
use context::PluginContext;
use tools_core::FeatureMigration;

/// A feature that participates in full application initialization.
///
/// Unlike `tools_core::FeaturePackage` (which defines tool metadata and migrations),
/// `AppCorePlugin` controls lifecycle: active registration during init, background
/// services, and event translation.
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
