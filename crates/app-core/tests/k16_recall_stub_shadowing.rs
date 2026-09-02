//! K16 — Recall stub shadowing.
//!
//! Invariant: registering a "live" tool with the same name as a previously
//! registered stub causes `ToolRegistry::get(name)` to return the live tool.
//! This protects against init-order regressions that would silently leave
//! the agent talking to a `[recall stub: …]` after Sprint-A T1 wired the
//! real `CodingMemoryToolset` (see crates/app-core/src/init/mod.rs).
//!
//! Mirrors the K15 layout: tiny model + proptest over inputs.

use std::sync::Arc;

use async_trait::async_trait;
use proptest::prelude::*;
use serde_json::Value;
use tools_core::{Tool, ToolRegistry};

const LIVE_MARKER: &str = "live-recall-tool";
const STUB_MARKER: &str = "stub-recall-tool";

const RECALL_NAMES: &[&str] = &[
    "recall_index",
    "recall_timeline",
    "recall_fetch",
    "trace_causes",
    "check_dead_ends",
    "recall_facts_as_of",
    "recall_change_history",
    "recall_decision_points",
];

struct FakeTool {
    name: &'static str,
    marker: &'static str,
}

#[async_trait]
impl Tool for FakeTool {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        self.marker
    }
    fn parameters(&self) -> Value {
        serde_json::json!({ "type": "object" })
    }
    async fn execute(
        &self,
        _args: Value,
        _ctx: &tools_core::RoutingContext,
    ) -> common::Result<String> {
        Ok(self.marker.into())
    }
    fn is_concurrency_safe(&self, _args: &Value) -> bool {
        true
    }
}

fn register(reg: &mut ToolRegistry, name: &'static str, marker: &'static str) {
    reg.register_dyn(Arc::new(FakeTool { name, marker }));
}

/// The property: whichever marker was registered LAST for a given name wins.
fn shadow_property(stub_first: bool) -> bool {
    let mut reg = ToolRegistry::new();
    let (first, second) = if stub_first {
        (STUB_MARKER, LIVE_MARKER)
    } else {
        (LIVE_MARKER, STUB_MARKER)
    };
    for &n in RECALL_NAMES {
        register(&mut reg, n, first);
    }
    for &n in RECALL_NAMES {
        register(&mut reg, n, second);
    }
    RECALL_NAMES.iter().all(|n| {
        reg.get(n)
            .map(|t| t.description() == second)
            .unwrap_or(false)
    })
}

proptest! {
    #[test]
    fn k16_last_registration_wins(stub_first in any::<bool>()) {
        prop_assert!(shadow_property(stub_first));
    }
}

#[test]
fn k16_live_after_stub_shadows_concretely() {
    // Concrete spot-check using the real recall_stubs entries from klynt-core,
    // shadowed by FakeTool acting as the "live" CodingMemoryToolset stand-in.
    let mut reg = ToolRegistry::new();
    reg.register_dyn(Arc::new(klynt_core::tools::recall_stubs::RecallIndexTool));
    reg.register_dyn(Arc::new(FakeTool {
        name: "recall_index",
        marker: LIVE_MARKER,
    }));
    let got = reg.get("recall_index").expect("recall_index present");
    assert_eq!(got.description(), LIVE_MARKER, "live tool must shadow stub");
}
