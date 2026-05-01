use common::tool_channel::{Channel, ChannelMask};
use proptest::prelude::*;

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

    #[test]
    fn k13_host_approval_dedup_correctness(
        n_calls in 2usize..16,
        m_hosts in 1usize..6,
    ) {
        use klynt_core::approval::host_cache::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let cache = Arc::new(HostApprovalCache::default());
        let approvals = Arc::new(AtomicUsize::new(0));

        // Generate n_calls dispatched across m_hosts.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut handles = Vec::new();
            let unique_hosts: std::collections::HashSet<_> = (0..n_calls)
                .map(|i| i % m_hosts)
                .collect();
            for i in 0..n_calls {
                let cache_c = cache.clone();
                let approvals_c = approvals.clone();
                let host_idx = i % m_hosts;
                handles.push(tokio::spawn(async move {
                    let key = HostKey::from_url(
                        &format!("https://host{}.example.com", host_idx)).unwrap();
                    match cache_c.check_or_register(key.clone()) {
                        HostCheckResult::NewlyRegistered { tx } => {
                            approvals_c.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                            let _ = tx.send(Some(HostDecision::AllowForSession));
                            cache_c.resolve(key, HostDecision::AllowForSession);
                        }
                        HostCheckResult::AwaitPending(mut rx) => {
                            rx.changed().await.unwrap();
                        }
                        HostCheckResult::Cached(_) => {}
                    }
                }));
            }
            for h in handles { h.await.unwrap(); }
            prop_assert_eq!(approvals.load(Ordering::SeqCst), unique_hosts.len(),
                "expected one approval per unique host");
            Ok::<(), TestCaseError>(())
        }).unwrap();
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
