//! E2E: open coding thread → user asks question → recall_index injects snippets
//! into system prompt → agent receives them in next iteration.

#[tokio::test]
#[ignore = "requires coding-memory phase 4 + LLM mock"]
async fn recall_injection_appears_in_system_prompt() {
    // Stub test — full E2E requires:
    // 1. Seeded coding-memory facts
    // 2. AppCore::for_test() with cognitive tables migrated
    // 3. A way to capture the assembled system prompt
    // 4. LLM mock so no real provider call is made
    //
    // When enabled, the test should:
    //   let core = AppCore::for_test().await.unwrap();
    //   core.coding_memory_seed_fact_for_test("repo-x", "main parser uses nom 7").await.unwrap();
    //   let thread = core.create_coding_thread("repo-x", "/tmp/repo-x").await.unwrap();
    //   let recorded = core.chat_send_capture_system_prompt(...).await.unwrap();
    //   assert!(recorded.system_prompt.contains("nom 7"));
    //   assert!(recorded.events.iter().any(|e| matches!(e, agent::events::AgentEvent::RecallInjected { .. })));
}
