//! PlanModeInjector — pushes a per-turn `<system-reminder>` while plan mode is active.

use std::path::PathBuf;
use std::sync::Arc;

use bus::context_updates::{ContextUpdate, ContextUpdateReason, UpdatePriority};
use bus::{DynamicInjector, InjectorContext};
use dashmap::DashMap;
use parking_lot::RwLock;

use crate::render;
use approval::CodingApprovalPolicy;

/// Looks up the per-thread approval policy and emits a plan-mode reminder
/// when applicable.
pub struct PlanModeInjector {
    policies: Arc<DashMap<String, Arc<RwLock<CodingApprovalPolicy>>>>,
}

impl PlanModeInjector {
    pub fn new(policies: Arc<DashMap<String, Arc<RwLock<CodingApprovalPolicy>>>>) -> Self {
        Self { policies }
    }
}

impl DynamicInjector for PlanModeInjector {
    fn name(&self) -> &str { "plan_mode" }

    fn collect(&self, ctx: &dyn InjectorContext) -> Vec<ContextUpdate> {
        if !ctx.plan_mode_active() { return Vec::new(); }
        let Some(lock) = self.policies.get(ctx.thread_id()) else { return Vec::new(); };
        let policy = lock.read();
        let (slug, path): (String, PathBuf) = match &*policy {
            CodingApprovalPolicy::PlanMode { plan_file_slug, plan_file_path, .. } => {
                (plan_file_slug.clone(), plan_file_path.clone())
            }
            _ => return Vec::new(),
        };
        let prose = render::plan_mode_reminder(&slug, &path);
        vec![ContextUpdate {
            reason: ContextUpdateReason::Custom("plan_mode_active".into()),
            content: Some(prose),
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: jiff::Timestamp::now(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approval::CodingApprovalPolicy;
    use config::schema::DefaultPolicy;

    struct FakeCtx { active: bool, thread: String }
    impl InjectorContext for FakeCtx {
        fn thread_id(&self) -> &str { &self.thread }
        fn agent_id(&self) -> &str { "root" }
        fn plan_mode_active(&self) -> bool { self.active }
        fn plan_session_id(&self) -> Option<&str> { Some("p_abc") }
    }

    fn make_plan_policy() -> CodingApprovalPolicy {
        let perms = config::schema::CodingPermissions {
            allow: vec![], deny: vec![], ask: vec![],
            default_if_no_match: DefaultPolicy::Ask,
            mirror_learning: false, mirror_min_approvals: 5, mirror_cooldown_hours: 24,
        };
        let base = CodingApprovalPolicy::compile(&perms).unwrap();
        let (allow, deny, ask, default_if_no_match) = match base {
            CodingApprovalPolicy::Default { allow, deny, ask, default_if_no_match } => (allow, deny, ask, default_if_no_match),
            _ => unreachable!(),
        };
        CodingApprovalPolicy::PlanMode {
            plan_session_id: "p_abc".into(),
            plan_file_slug: "2026-05-08-test".into(),
            plan_file_path: "/tmp/2026-05-08-test.md".into(),
            allow, deny, ask, default_if_no_match,
        }
    }

    #[test]
    fn returns_empty_when_off() {
        let injector = PlanModeInjector::new(Arc::new(DashMap::new()));
        let out = injector.collect(&FakeCtx { active: false, thread: "t1".into() });
        assert!(out.is_empty());
    }

    #[test]
    fn returns_empty_when_no_policy_for_thread() {
        let injector = PlanModeInjector::new(Arc::new(DashMap::new()));
        let out = injector.collect(&FakeCtx { active: true, thread: "missing".into() });
        assert!(out.is_empty());
    }

    #[test]
    fn returns_one_update_when_active() {
        let policies = Arc::new(DashMap::new());
        policies.insert("t1".into(), Arc::new(RwLock::new(make_plan_policy())));
        let injector = PlanModeInjector::new(policies);
        let out = injector.collect(&FakeCtx { active: true, thread: "t1".into() });
        assert_eq!(out.len(), 1);
        let content = out[0].content.as_ref().unwrap();
        assert!(content.contains("Plan mode active"));
        assert!(content.contains("2026-05-08-test"));
    }
}
