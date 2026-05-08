//! DynamicInjector — pluggable producers of `<system-reminder>` updates,
//! drained by the agent's LiveContextRefresher each iteration.
//!
//! Reusable for: plan mode (this phase), Phase 2.4 hooks (PreToolUse / PostToolUse / PreCompact).

use std::sync::Arc;

use crate::context_updates::ContextUpdate;

/// Trait an injector implements to push per-turn context updates.
pub trait DynamicInjector: Send + Sync {
    /// Stable name used for tracing.
    fn name(&self) -> &str;
    /// Collect zero or more `ContextUpdate`s for the current routing state.
    /// Implementations are expected to be cheap; called once per LLM iteration.
    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate>;
}

/// Minimal abstraction over RoutingContext to keep `bus` decoupled from `tools-core`.
/// (RoutingContext is in tools-core; bus is L1 and tools-core is L1 — but coupling
/// the two creates a dep cycle. So we shape this as a trait that tools-core's
/// RoutingContext implements via a thin adapter.)
pub trait InjectorContext: Send + Sync {
    fn thread_id(&self) -> &str;
    fn agent_id(&self) -> &str;
    fn plan_mode_active(&self) -> bool;
    fn plan_session_id(&self) -> Option<&str>;
}

/// Holds the set of registered injectors. Cheap to clone (Arc-wrapped Vec).
#[derive(Clone, Default)]
pub struct InjectorRegistry {
    injectors: Arc<Vec<Arc<dyn DynamicInjector>>>,
}

impl std::fmt::Debug for InjectorRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectorRegistry")
            .field("names", &self.names())
            .finish()
    }
}

impl InjectorRegistry {
    pub fn new(injectors: Vec<Arc<dyn DynamicInjector>>) -> Self {
        Self {
            injectors: Arc::new(injectors),
        }
    }

    pub fn empty() -> Self {
        Self {
            injectors: Arc::new(Vec::new()),
        }
    }

    /// Drive every registered injector and concatenate their updates.
    pub fn collect_all(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        self.injectors.iter().flat_map(|i| i.collect(ctx)).collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.injectors.iter().map(|i| i.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
    use jiff::Timestamp;

    struct FakeCtx {
        plan: bool,
    }
    impl InjectorContext for FakeCtx {
        fn thread_id(&self) -> &str {
            "t1"
        }
        fn agent_id(&self) -> &str {
            "root"
        }
        fn plan_mode_active(&self) -> bool {
            self.plan
        }
        fn plan_session_id(&self) -> Option<&str> {
            if self.plan {
                Some("p_abc")
            } else {
                None
            }
        }
    }

    struct AlwaysOne;
    impl DynamicInjector for AlwaysOne {
        fn name(&self) -> &str {
            "always_one"
        }
        fn collect(&self, _ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
            vec![ContextUpdate {
                reason: ContextUpdateReason::Custom("test".into()),
                content: Some("hello".into()),
                metadata: None,
                priority: UpdatePriority::High,
                timestamp: Timestamp::now(),
            }]
        }
    }

    #[test]
    fn empty_registry_yields_nothing() {
        let reg = InjectorRegistry::empty();
        let updates = reg.collect_all(&FakeCtx { plan: false });
        assert!(updates.is_empty());
    }

    #[test]
    fn registry_runs_each_injector() {
        let reg = InjectorRegistry::new(vec![Arc::new(AlwaysOne), Arc::new(AlwaysOne)]);
        let updates = reg.collect_all(&FakeCtx { plan: true });
        assert_eq!(updates.len(), 2);
    }

    #[test]
    fn registry_lists_injector_names() {
        let reg = InjectorRegistry::new(vec![Arc::new(AlwaysOne)]);
        assert_eq!(reg.names(), vec!["always_one"]);
    }
}
