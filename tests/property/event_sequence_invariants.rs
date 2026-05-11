//! Proptest invariant: any permutation of (send, cancel, error, complete)
//! across N sessions leaves the AppCore active_streams empty.

use proptest::prelude::*;
use proptest::strategy::Strategy;

#[derive(Debug, Clone)]
enum Op {
    Send(u8),
    Cancel(u8),
    SimulateError(u8),
    SimulateComplete(u8),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..5).prop_map(Op::Send),
        (0u8..5).prop_map(Op::Cancel),
        (0u8..5).prop_map(Op::SimulateError),
        (0u8..5).prop_map(Op::SimulateComplete),
    ]
}

fn op_session_id(op: &Op) -> u8 {
    match op {
        Op::Send(id) | Op::Cancel(id) | Op::SimulateError(id) | Op::SimulateComplete(id) => *id,
    }
}

#[cfg(not(feature = "soak"))]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// CHAT-INV-1: after any sequence of operations, active_streams is empty
    /// (eventually, modulo a small drain window).
    #[test]
    fn active_streams_drains(ops in proptest::collection::vec(op_strategy(), 0..20)) {
        run_active_streams_drains(ops)?;
    }
}

#[cfg(feature = "soak")]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// Soak variant: 10,000 random op-sequences.
    #[test]
    fn active_streams_drains_soaked(ops in proptest::collection::vec(op_strategy(), 0..20)) {
        run_active_streams_drains(ops)?;
    }
}

fn run_active_streams_drains(ops: Vec<Op>) -> Result<(), proptest::test_runner::TestCaseError> {
    // Use a multi-thread runtime with an increased stack size.
    // AppCore::for_tests() involves deep init (migrations, hook engine,
    // provider wiring) that can overflow the default 2 MB test thread stack.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        // Spawn the heavy initialization onto a worker thread so it runs
        // with the enlarged stack rather than the test harness thread.
        let (core, emitter) = tokio::task::spawn(async move {
            crate::common::chat_harness::ChatTestHarness::new_real().await
        })
        .await
        .unwrap();
        for op in &ops {
            let sk = format!("test:proptest:{:?}", op_session_id(op));
            match op {
                Op::Send(_) => {
                    if let Ok((_resp, info)) = core.chat_send("x".into(), sk, None, None).await {
                        core.spawn_chat_relay(info, emitter.clone());
                    }
                }
                Op::Cancel(_) => {
                    let _ = core.chat_cancel(sk).await;
                }
                Op::SimulateError(_) | Op::SimulateComplete(_) => {
                    // Deferred to Task 42 (event-injection harness).
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Wait for drain
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        prop_assert_eq!(core.active_streams_len(), 0, "active_streams must drain");
        Ok(())
    })?;
    Ok(())
}
