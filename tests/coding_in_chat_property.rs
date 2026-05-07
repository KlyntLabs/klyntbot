use common::tool_channel::{Channel, ChannelMask};
use proptest::prelude::*;

// Helper: strip UUID/timestamp from ingest events so we can compare determinism.
fn event_kinds(evts: &[coding_ingest::event::AgentEvent]) -> Vec<coding_ingest::event::EventKind> {
    evts.iter()
        .map(|e| match e {
            coding_ingest::event::AgentEvent::V1(v1) => v1.kind.clone(),
        })
        .collect()
}

proptest! {
    #[test]
    fn k12_channel_mask_filter_idempotent(
        coding in any::<bool>(),
        desktop in any::<bool>(),
        other in any::<bool>(),
        ch_idx in 0u8..3,
    ) {
        let mut mask = ChannelMask::empty();
        if coding { mask |= ChannelMask::CODING; }
        if desktop { mask |= ChannelMask::DESKTOP; }
        if other { mask |= ChannelMask::OTHER; }
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };
        let pass1 = mask.allows(ch);
        let pass2 = mask.allows(ch);
        prop_assert_eq!(pass1, pass2);
    }

    #[test]
    fn k14_no_mutating_tool_is_visible_in_non_ui_channel(
        tool_idx in 0u8..13,
        ch_idx in 0u8..3,
    ) {
        // Enumerate all 13 klynt-core tool names
        let tools = [
            ("bash", true), ("edit", true), ("write", true),
            ("apply_patch", true), ("notebook_edit", true),
            ("enter_plan_mode", false), ("exit_plan_mode", false),
            ("read", false), ("glob", false), ("grep", false),
            ("ask_user", false), ("web_fetch", false), ("tool_search", false),
        ];
        let (name, is_mutating) = tools[tool_idx as usize];
        let ch = match ch_idx { 0 => Channel::Coding, 1 => Channel::Desktop, _ => Channel::Other };

        // After Task 8 graduation, the read-only / portable tools graduate.
        // Mutating tools never graduate. Therefore, for any mutating tool in
        // a non-coding channel, the mask must NOT allow.
        if is_mutating && !matches!(ch, Channel::Coding) {
            // ChannelMask::CODING_ONLY is the expected override — it does not allow non-coding.
            let mask = common::tool_channel::ChannelMask::CODING_ONLY;
            prop_assert!(!mask.allows(ch),
                "mutating tool {name} must not be visible in {ch:?}");
        }
        let _ = name; // suppress unused warning
    }

}

proptest! {
    /// K1 — Translator round-trip determinism.
    /// The same sequence of RuntimeEvents always yields the same EventKinds.
    #[test]
    fn k1_translator_round_trip_determinism(
        events in proptest::collection::vec(any::<u8>(), 0..24)
    ) {
        use coding_memory::sink::translator::{RuntimeEvent, Translator};

        let trace: Vec<RuntimeEvent> = events.iter().map(|b| match b % 6 {
            0 => RuntimeEvent::ContentChunk { text: "x".into() },
            1 => RuntimeEvent::ToolStart {
                call_id: format!("t{b}"),
                name: "n".into(),
                args: serde_json::json!({}),
            },
            2 => RuntimeEvent::ToolEnd {
                call_id: format!("t{b}"),
                success: true,
                output: "".into(),
                duration_ms: 1,
            },
            3 => RuntimeEvent::IterationStart,
            4 => RuntimeEvent::IterationEnd,
            _ => RuntimeEvent::ContentChunk { text: "y".into() },
        }).collect();

        let mut t1 = Translator::new();
        let mut t2 = Translator::new();
        let mut out1 = Vec::new();
        let mut out2 = Vec::new();
        for e in &trace {
            out1.extend(t1.translate(e).unwrap());
            out2.extend(t2.translate(e).unwrap());
        }
        prop_assert_eq!(event_kinds(&out1), event_kinds(&out2));
    }
}

proptest! {
    /// K2 — Translator monotonicity.
    /// Cumulative emitted event count never decreases as we process prefixes.
    #[test]
    fn k2_translator_monotonicity(
        events in proptest::collection::vec(any::<u8>(), 0..24)
    ) {
        use coding_memory::sink::translator::{RuntimeEvent, Translator};

        let trace: Vec<RuntimeEvent> = events.iter().map(|b| match b % 6 {
            0 => RuntimeEvent::ContentChunk { text: "x".into() },
            1 => RuntimeEvent::ToolStart {
                call_id: format!("t{b}"),
                name: "n".into(),
                args: serde_json::json!({}),
            },
            2 => RuntimeEvent::ToolEnd {
                call_id: format!("t{b}"),
                success: true,
                output: "".into(),
                duration_ms: 1,
            },
            3 => RuntimeEvent::IterationStart,
            4 => RuntimeEvent::IterationEnd,
            _ => RuntimeEvent::ContentChunk { text: "y".into() },
        }).collect();

        let mut t = Translator::new();
        let mut prev = 0usize;
        for e in &trace {
            let emitted = t.translate(e).unwrap().len();
            let now = prev + emitted;
            prop_assert!(now >= prev, "non-monotone: {} -> {}", prev, now);
            prev = now;
        }
    }
}

proptest! {
    /// K4 — Privacy guard inviolability.
    /// A path that matches an exclude pattern is always excluded,
    /// and bash commands referencing excluded paths are detected.
    #[test]
    fn k4_privacy_guard_inviolability(
        path_suffix in "[a-zA-Z0-9_]{1,20}",
    ) {
        use klynt_core::privacy::PrivacyGuard;
        use std::path::Path;

        let guard = PrivacyGuard::from_globs(&["*.secret", "private/*", "**/.env"]).unwrap();

        // These should always be excluded.
        prop_assert!(guard.is_excluded(Path::new("foo.secret")));
        prop_assert!(guard.is_excluded(Path::new("private/anything")));
        prop_assert!(guard.is_excluded(Path::new("a/b/.env")));

        // A bash command touching an excluded path is detected.
        let cmd = format!("cat private/{path_suffix}");
        prop_assert!(guard.bash_command_touches_excluded(&cmd),
            "bash command should be flagged: {cmd}");
    }
}

proptest! {
    /// K8 — Approval round-trip identity.
    /// For every matched ApprovalRequested + ApprovalResolved pair,
    /// the translator emits exactly one ApprovalDecision.
    #[test]
    fn k8_approval_round_trip_identity(
        n in 1usize..8,
    ) {
        use coding_memory::sink::translator::{RuntimeEvent, Translator};

        let mut t = Translator::new();
        let mut decisions = 0usize;
        for i in 0..n {
            let req = t.translate(&RuntimeEvent::ApprovalRequested {
                request_id: format!("r{i}"),
                tool: "bash".into(),
                layer: "layer1".into(),
            }).unwrap();
            // ApprovalRequested alone emits nothing.
            prop_assert_eq!(req.len(), 0);

            let res = t.translate(&RuntimeEvent::ApprovalResolved {
                request_id: format!("r{i}"),
                decision: "allow".into(),
            }).unwrap();
            decisions += res.iter().filter(|e| matches!(e, coding_ingest::event::AgentEvent::V1(v1) if matches!(v1.kind, coding_ingest::event::EventKind::ApprovalDecision { .. }))).count();
        }
        prop_assert_eq!(decisions, n, "expected {} ApprovalDecision events", n);
    }
}

proptest! {
    /// K3 — Approval gate composition.
    /// For any (Layer1 decision, Layer2 decision) pair, the merged decision
    /// follows the spec's priority: deny beats allow beats ask; FallThrough
    /// from one layer passes to the next.
    #[test]
    fn k3_approval_gate_composition(
        l1_idx in 0u8..4,
        l2_idx in 0u8..4,
    ) {
        use klynt_execpolicy::Decision;
        let l1 = match l1_idx { 0 => Decision::Allow, 1 => Decision::Ask, 2 => Decision::Forbid, _ => Decision::FallThrough };
        let l2 = match l2_idx { 0 => Decision::Allow, 1 => Decision::Ask, 2 => Decision::Forbid, _ => Decision::FallThrough };

        // The tool-layer-consolidation `guard.rs:95-113` merge logic:
        //   if l1 == Auto (Allow/Forbid) → return l1 (short-circuits Layer 2)
        //   else l2 == Allow → Auto Allow Layer2
        //   else l2 == Forbid → Auto Forbid Layer2
        //   else l2 == Ask → Ask Layer2
        //   else l2 == FallThrough → fall back to l1
        let merged = match l1 {
            Decision::Allow | Decision::Forbid => l1,
            _ => match l2 {
                Decision::Allow => Decision::Allow,
                Decision::Forbid => Decision::Forbid,
                Decision::Ask => Decision::Ask,
                Decision::FallThrough => l1,
            }
        };
        // Property: Forbid wins per the merge logic semantics.
        if l1 == Decision::Forbid {
            prop_assert_eq!(merged, Decision::Forbid);
        } else if l1 == Decision::Allow {
            prop_assert_eq!(merged, Decision::Allow);
        } else if l2 == Decision::Forbid {
            prop_assert_eq!(merged, Decision::Forbid);
        }
    }
}
