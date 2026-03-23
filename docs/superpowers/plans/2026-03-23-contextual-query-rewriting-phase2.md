# Contextual Query Rewriting Phase 2 — LLM Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add LLM fallback to the contextual query rewriter for Low-specificity queries (pronouns, time references, coaching recall) that the heuristic can't resolve, enabling Moments 1, 3, 5, and 8.

**Architecture:** When `specificity=Low` and heuristic returns `None`, fire a small/fast LLM call (Haiku-class) with 800ms timeout. The LLM receives the original query + context signals and produces a rewritten search query. Uses the background race pattern: heuristic result (if any) goes to InsightForge immediately; LLM races in background and injects a late sub-query if it finishes in time.

**Tech Stack:** Rust, tokio (spawn, timeout, select, oneshot), async_trait, existing `DynProvider` + `ChatParams` from `providers` crate.

**Spec:** `docs/superpowers/specs/2026-03-23-contextual-query-rewriting-design.md` (sections: "LLM fallback prompt", "LLM fallback — background race pattern", "Phase 2")

**Phase 1 plan:** `docs/superpowers/plans/2026-03-23-contextual-query-rewriting.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/config/src/schema/agents.rs` | Modify | Add `rewriter_model: Option<String>` to `AgentsConfig` |
| `crates/agent/src/adapters/query_rewriter.rs` | Modify | Add `llm_rewrite()` method, update `rewrite()` to call it on Low+heuristic-None |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Pass `cognitive_provider` + config model to `ContextualQueryRewriter::new()` |
| `crates/context_engine/src/assembler/mod.rs` | Modify | Implement background race: spawn LLM rewrite concurrently with InsightForge |
| `crates/context_engine/src/rewriter.rs` | Modify | Extend `QueryRewriter` trait with `rewrite_background()` for the race pattern |

---

## Task 1: Add `rewriterModel` config field

**Files:**
- Modify: `crates/config/src/schema/agents.rs:6-22`

- [ ] **Step 1: Add field to `AgentsConfig`**

In `AgentsConfig` struct, add after `skills_dir`:

```rust
    /// Model to use for query rewriting LLM fallback (Phase 2).
    /// Defaults to the cheapest/fastest available model. Example: "anthropic/claude-haiku-4-5"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewriter_model: Option<String>,
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p config`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add crates/config/src/schema/agents.rs
git commit -m "feat(config): add rewriterModel config for query rewriting LLM fallback"
```

---

## Task 2: Implement `llm_rewrite()` in `ContextualQueryRewriter`

**Files:**
- Modify: `crates/agent/src/adapters/query_rewriter.rs:286-444`

This is the core Phase 2 implementation.

- [ ] **Step 1: Write tests for LLM fallback behavior**

Add to the existing test module (after line ~447):

```rust
    // --- Phase 2 LLM fallback tests ---

    use providers::{LlmResponse, Message, ChatParams};

    /// Mock provider that returns a configurable rewrite string.
    struct MockRewriteProvider {
        response: String,
        delay_ms: u64,
    }

    impl MockRewriteProvider {
        fn new(response: &str) -> Self {
            Self { response: response.into(), delay_ms: 0 }
        }
        fn slow(response: &str, delay_ms: u64) -> Self {
            Self { response: response.into(), delay_ms }
        }
    }

    #[async_trait::async_trait]
    impl providers::LlmProvider for MockRewriteProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            if self.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            }
            Ok(LlmResponse {
                content: self.response.clone(),
                tool_calls: vec![],
                usage: providers::Usage::default(),
                stop_reason: None,
                raw_response: None,
            })
        }
        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<providers::LlmStream> {
            unimplemented!()
        }
        fn supports_streaming(&self) -> bool { false }
        fn model_name(&self) -> &str { "mock" }
    }

    #[tokio::test]
    async fn llm_fallback_on_pronoun_no_heuristic_context() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::new("auth middleware refactoring compliance changes")
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        // Low specificity (pronouns), no context for heuristic → LLM fires
        let ctx = RetrievalContext {
            recent_user_messages: vec!["we discussed the auth middleware refactor".into()],
            ..Default::default()
        };
        let result = rewriter.rewrite("what was that thing?", &ctx).await;
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.enriched_query.contains("auth middleware"));
        assert_eq!(r.source, RewriteSource::Llm);
        assert!(r.confidence >= 0.6);
    }

    #[tokio::test]
    async fn llm_skip_response_returns_none() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::new("SKIP")
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        let ctx = RetrievalContext::default();
        let result = rewriter.rewrite("what was that?", &ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn llm_timeout_returns_none() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::slow("this should timeout", 2000)
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 100); // 100ms timeout
        let ctx = RetrievalContext::default();
        let result = rewriter.rewrite("what was that?", &ctx).await;
        assert!(result.is_none()); // Should timeout and return None
    }

    #[tokio::test]
    async fn llm_not_called_when_heuristic_succeeds() {
        // Even with a provider, if heuristic enriches, LLM should NOT fire
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::new("LLM was called — this is wrong!")
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        let ctx = RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "March budget review".into(),
                project_name: None,
                domain: Some("finance".into()),
            }),
            ..Default::default()
        };
        // "what about that?" is Low specificity but heuristic has context → heuristic wins
        let result = rewriter.rewrite("what about that?", &ctx).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().source, RewriteSource::Heuristic);
    }

    #[tokio::test]
    async fn llm_not_called_for_medium_specificity() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::new("LLM was called — wrong!")
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        let ctx = RetrievalContext::default(); // no context for heuristic
        // Medium specificity + empty context → None (no LLM for Medium)
        let result = rewriter.rewrite("tell me about the current progress on everything", &ctx).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn llm_no_provider_degrades_gracefully() {
        // No provider → LLM path skipped, returns None
        let rewriter = ContextualQueryRewriter::new(None, None, 800);
        let ctx = RetrievalContext::default();
        let result = rewriter.rewrite("what was that?", &ctx).await;
        assert!(result.is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(llm_fallback) + test(llm_skip) + test(llm_timeout) + test(llm_not_called) + test(llm_no_provider)'`
Expected: FAIL (llm_rewrite not implemented yet)

- [ ] **Step 3: Implement `llm_rewrite()` method**

Add to `ContextualQueryRewriter` impl block (after `heuristic_rewrite`):

```rust
    /// Phase 2: LLM fallback for Low-specificity queries when heuristic has no context.
    async fn llm_rewrite(&self, original: &str, context: &RetrievalContext) -> Option<RewriteResult> {
        let provider = self.llm_provider.as_ref()?;

        let recent = context.recent_user_messages
            .iter()
            .take(2)
            .map(|m| m.chars().take(100).collect::<String>())
            .collect::<Vec<_>>()
            .join("; ");

        let prompt = format!(
            "You are a query rewriter for a personal AI assistant. Given the user's \
             vague query and their current context, produce a single enriched search \
             query that captures what they likely mean.\n\n\
             Rules:\n\
             - Output ONLY the rewritten query, nothing else\n\
             - Keep it under 20 words\n\
             - Preserve any time references from the original\n\
             - If the query is already clear enough, output \"SKIP\"\n\n\
             User's query: \"{original}\"\n\n\
             Context:\n\
             - Active skill: {skill}\n\
             - Current task: {task}\n\
             - Recent messages: {recent}\n\
             - Current view: {view}\n\n\
             Rewritten query:",
            original = original,
            skill = context.active_skill.as_deref().unwrap_or("none"),
            task = context.active_task.as_ref().map(|t| t.title.as_str()).unwrap_or("none"),
            recent = if recent.is_empty() { "none".to_string() } else { recent },
            view = context.active_view.as_ref()
                .and_then(|v| v.description.as_deref())
                .unwrap_or("none"),
        );

        let messages = vec![providers::Message::user(prompt)];
        let params = providers::ChatParams {
            model: self.rewriter_model.clone(),
            max_tokens: Some(50),
            temperature: Some(0.0),
            ..Default::default()
        };

        let timeout_dur = std::time::Duration::from_millis(self.timeout_ms);
        let result = tokio::time::timeout(timeout_dur, provider.chat(&messages, None, &params)).await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content.trim().to_string();
                if text.eq_ignore_ascii_case("SKIP") || text.is_empty() {
                    debug!(original = original, "⏭️ QueryRewriter: LLM returned SKIP");
                    None
                } else {
                    debug!(
                        original = original,
                        enriched = text.as_str(),
                        "✅ QueryRewriter: LLM enriched query"
                    );
                    Some(RewriteResult {
                        enriched_query: text,
                        confidence: 0.75,
                        source: RewriteSource::Llm,
                    })
                }
            }
            Ok(Err(e)) => {
                debug!(original = original, error = %e, "⚠️ QueryRewriter: LLM call failed");
                None
            }
            Err(_) => {
                debug!(original = original, timeout_ms = self.timeout_ms, "⏱️ QueryRewriter: LLM timed out");
                None
            }
        }
    }
```

- [ ] **Step 4: Update `rewrite()` to call LLM fallback**

Change the match in `rewrite()` from:

```rust
        let result = match specificity {
            Specificity::High => None,
            Specificity::Medium | Specificity::Low => {
                self.heuristic_rewrite(original, context)
                // TODO Phase 2: LLM fallback when heuristic returns None for Low specificity
            }
        };
```

To:

```rust
        let result = match specificity {
            Specificity::High => None,
            Specificity::Medium => self.heuristic_rewrite(original, context),
            Specificity::Low => {
                if let Some(heuristic) = self.heuristic_rewrite(original, context) {
                    Some(heuristic)
                } else {
                    self.llm_rewrite(original, context).await
                }
            }
        };
```

- [ ] **Step 5: Remove `#[allow(dead_code)]` from struct fields**

The fields `llm_provider`, `rewriter_model`, `timeout_ms` are no longer dead code. Remove the three `#[allow(dead_code)]` annotations from the struct definition.

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p agent -E 'test(query_rewriter) + test(specificity) + test(llm_)'`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/adapters/query_rewriter.rs
git commit -m "feat(agent): add LLM fallback to ContextualQueryRewriter (Phase 2)"
```

---

## Task 3: Wire LLM provider + config into builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:817-820`

- [ ] **Step 1: Change rewriter construction to pass provider**

Replace the current heuristic-only construction at line ~819:

```rust
        // Was:
        let query_rewriter: Arc<dyn context_engine::QueryRewriter> = Arc::new(
            crate::adapters::query_rewriter::ContextualQueryRewriter::heuristic_only()
        );
```

With:

```rust
        // Phase 2: Wire LLM provider + config model for query rewriting fallback
        let rewriter_provider = self.cognitive_provider.clone();
        let rewriter_model = config.agents.rewriter_model.clone();
        let rewriter_timeout = 800; // 800ms hard cap per spec
        let query_rewriter: Arc<dyn context_engine::QueryRewriter> = Arc::new(
            crate::adapters::query_rewriter::ContextualQueryRewriter::new(
                rewriter_provider,
                rewriter_model,
                rewriter_timeout,
            )
        );
```

This reuses the existing `cognitive_provider` (already available in the builder at line 82) as the LLM for query rewriting. The `rewriter_model` comes from config — if `None`, the provider uses its default model.

- [ ] **Step 2: Verify compilation + tests**

Run: `cargo check --workspace && cargo nextest run -p agent`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire LLM provider and config into QueryRewriter builder"
```

---

## Task 4: Implement background race pattern

**Files:**
- Modify: `crates/context_engine/src/rewriter.rs` (extend trait)
- Modify: `crates/context_engine/src/assembler/mod.rs:391-418` (race logic)
- Modify: `crates/agent/src/adapters/query_rewriter.rs` (implement new trait method)

This is the most complex task — implementing the zero-perceived-latency race between the LLM rewrite and InsightForge.

- [ ] **Step 1: Add `rewrite_or_spawn` method to `QueryRewriter` trait**

In `crates/context_engine/src/rewriter.rs`, add a new method to the trait with a default implementation:

```rust
    /// Attempt heuristic rewrite. If heuristic succeeds, return it immediately.
    /// If heuristic returns None and LLM is available, return None from this method
    /// but send the LLM result on the provided channel when it completes.
    /// The caller can race the channel against InsightForge.
    ///
    /// Default implementation: just calls rewrite() (no background work).
    async fn rewrite_or_spawn(
        &self,
        original: &str,
        context: &RetrievalContext,
        late_tx: Option<tokio::sync::oneshot::Sender<RewriteResult>>,
    ) -> Option<RewriteResult> {
        let result = self.rewrite(original, context).await;
        // Default: no background work, just drop the channel
        drop(late_tx);
        result
    }
```

- [ ] **Step 2: Implement `rewrite_or_spawn` in `ContextualQueryRewriter`**

In `crates/agent/src/adapters/query_rewriter.rs`, add to the `QueryRewriter` impl:

```rust
    async fn rewrite_or_spawn(
        &self,
        original: &str,
        context: &RetrievalContext,
        late_tx: Option<tokio::sync::oneshot::Sender<RewriteResult>>,
    ) -> Option<RewriteResult> {
        let specificity = query_specificity(original);

        // Log evaluation (same as rewrite())
        debug!(
            query = original,
            ?specificity,
            skill = context.active_skill.as_deref().unwrap_or("none"),
            "🔍 QueryRewriter: evaluating (race mode)"
        );

        match specificity {
            Specificity::High => {
                debug!(original = original, "⏭️ QueryRewriter: skipped (High)");
                None
            }
            Specificity::Medium => self.heuristic_rewrite(original, context),
            Specificity::Low => {
                // Try heuristic first
                if let Some(heuristic) = self.heuristic_rewrite(original, context) {
                    debug!(enriched = heuristic.enriched_query.as_str(), "✅ Heuristic enriched (race mode)");
                    return Some(heuristic);
                }
                // Heuristic failed — spawn LLM in background if channel provided
                if let (Some(tx), Some(provider)) = (late_tx, self.llm_provider.as_ref()) {
                    let provider = provider.clone();
                    let model = self.rewriter_model.clone();
                    let timeout_ms = self.timeout_ms;
                    let original = original.to_string();
                    let context = context.clone();
                    tokio::spawn(async move {
                        let rewriter = ContextualQueryRewriter::new(Some(provider), model, timeout_ms);
                        if let Some(result) = rewriter.llm_rewrite(&original, &context).await {
                            let _ = tx.send(result); // Ignore error if receiver dropped
                        }
                    });
                    debug!(original = original, "🚀 QueryRewriter: LLM spawned in background");
                }
                None // Return None immediately — InsightForge starts without enrichment
            }
        }
    }
```

- [ ] **Step 3: Update `retrieve_memory` to use the race pattern**

In `crates/context_engine/src/assembler/mod.rs`, replace the rewriter + InsightForge call:

```rust
    async fn retrieve_memory(&self, request: &ContextRequest) -> Option<(String, usize)> {
        let retriever = self.memory_retriever.as_ref()?;

        // Phase 2: Background race — heuristic immediate, LLM races with InsightForge
        let (enriched, late_rx) = match (&self.query_rewriter, &request.retrieval_context) {
            (Some(rewriter), Some(ctx)) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let result = rewriter.rewrite_or_spawn(&request.message_text, ctx, Some(tx)).await;
                if result.is_some() {
                    // Heuristic succeeded — no need for background LLM
                    (result, None)
                } else {
                    // Heuristic returned None — LLM may be running in background
                    (None, Some(rx))
                }
            }
            _ => (None, None),
        };

        if let Some(ref e) = enriched {
            tracing::debug!(
                enriched_query = e.enriched_query.as_str(),
                confidence = e.confidence,
                "🧠 ContextEngine: passing enriched query to InsightForge"
            );
        }

        // Start InsightForge (or plain retriever) — may race with background LLM
        let entries = if let Some(ref forge) = self.insight_forge {
            if forge.should_activate(&request.strategy, &request.message_text) {
                if let Some(late_rx) = late_rx {
                    // Race: InsightForge vs background LLM
                    // Pre-allocate a slot: start InsightForge with original only,
                    // then check if LLM finished and do a supplementary search
                    let forge_result = forge.retrieve_with_enrichment(
                        &request.message_text,
                        enriched.as_ref(),
                        self.memory_retrieval_limit,
                        request.session_key.as_deref(),
                    ).await;

                    // Check if LLM finished during InsightForge execution
                    match late_rx.try_recv() {
                        Ok(llm_result) => {
                            tracing::debug!(
                                enriched = llm_result.enriched_query.as_str(),
                                "🏁 LLM finished during InsightForge — doing supplementary search"
                            );
                            // Do a quick supplementary retrieval with the LLM query
                            let supplement = retriever
                                .retrieve(&llm_result.enriched_query, self.memory_retrieval_limit / 2)
                                .await;
                            // Merge: deduplicate by ID, keep forge results + new supplement entries
                            let existing_ids: std::collections::HashSet<String> =
                                forge_result.iter().map(|e| e.id.clone()).collect();
                            let mut merged = forge_result;
                            for entry in supplement {
                                if !existing_ids.contains(&entry.id) {
                                    merged.push(entry);
                                }
                            }
                            merged.truncate(self.memory_retrieval_limit);
                            merged
                        }
                        Err(_) => {
                            // LLM didn't finish in time — use forge results as-is
                            tracing::debug!("⏱️ LLM didn't finish during InsightForge — using original results");
                            forge_result
                        }
                    }
                } else {
                    // No background LLM — normal path (heuristic enrichment or no enrichment)
                    forge.retrieve_with_enrichment(
                        &request.message_text,
                        enriched.as_ref(),
                        self.memory_retrieval_limit,
                        request.session_key.as_deref(),
                    ).await
                }
            } else {
                retriever.retrieve(&request.message_text, self.memory_retrieval_limit).await
            }
        } else {
            retriever.retrieve(&request.message_text, self.memory_retrieval_limit).await
        };

        // ... rest of the method unchanged (formatting entries, etc.)
```

- [ ] **Step 4: Add `Clone` derive to `RetrievalContext` dependencies**

The background spawn needs to move context into the spawned task. `RetrievalContext` already derives `Clone`. Verify that all its fields are `Clone` — they should be since Phase 1 already set this up.

Run: `cargo check -p context_engine -p agent`
Expected: SUCCESS

- [ ] **Step 5: Write test for background race**

Add to the test module in `query_rewriter.rs`:

```rust
    #[tokio::test]
    async fn rewrite_or_spawn_returns_heuristic_immediately() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::slow("LLM result", 5000) // Very slow — shouldn't be waited for
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        let ctx = RetrievalContext {
            active_skill: Some("finance-management".into()),
            active_task: Some(ActiveTaskContext {
                title: "Budget review".into(),
                project_name: None,
                domain: Some("finance".into()),
            }),
            ..Default::default()
        };
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let result = rewriter.rewrite_or_spawn("how are we doing?", &ctx, Some(tx)).await;
        // Heuristic should succeed immediately — LLM not needed
        assert!(result.is_some());
        assert_eq!(result.unwrap().source, RewriteSource::Heuristic);
    }

    #[tokio::test]
    async fn rewrite_or_spawn_fires_llm_in_background() {
        let provider: providers::DynProvider = std::sync::Arc::new(
            MockRewriteProvider::new("auth middleware refactoring") // Fast LLM
        );
        let rewriter = ContextualQueryRewriter::new(Some(provider), None, 800);
        let ctx = RetrievalContext::default(); // No context for heuristic
        let (tx, rx) = tokio::sync::oneshot::channel();
        let result = rewriter.rewrite_or_spawn("what was that?", &ctx, Some(tx)).await;
        // Heuristic should fail (no context) → returns None, LLM spawned in background
        assert!(result.is_none());
        // Wait for background LLM
        let llm_result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx
        ).await;
        assert!(llm_result.is_ok());
        let r = llm_result.unwrap().unwrap();
        assert!(r.enriched_query.contains("auth middleware"));
        assert_eq!(r.source, RewriteSource::Llm);
    }
```

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p agent -E 'test(query_rewriter) + test(specificity) + test(llm_) + test(rewrite_or_spawn)'`
Expected: ALL PASS

Run: `cargo nextest run -p context_engine`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add crates/context_engine/src/rewriter.rs crates/context_engine/src/assembler/mod.rs crates/agent/src/adapters/query_rewriter.rs
git commit -m "feat(context-engine,agent): implement background LLM race pattern for query rewriting"
```

---

## Task 5: Final verification

- [ ] **Step 1: Run fmt**

Run: `cargo fmt --all --check`
Fix if needed: `cargo fmt --all`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 new warnings

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: ALL PASS

- [ ] **Step 4: Run doc tests**

Run: `cargo test --workspace --doc`
Expected: ALL PASS

- [ ] **Step 5: Commit if fixups needed**

```bash
git add -A && git commit -m "chore: clippy and fmt fixes for Phase 2 query rewriting"
```

---

## Summary

| Task | Description | Lines | Dependencies |
|------|-------------|-------|-------------|
| 1 | Config field `rewriterModel` | ~5 | None |
| 2 | `llm_rewrite()` + updated `rewrite()` + 6 tests | ~150 | Task 1 |
| 3 | Wire provider + config in builder | ~10 | Tasks 1, 2 |
| 4 | Background race pattern (`rewrite_or_spawn` + assembler race) | ~120 | Tasks 2, 3 |
| 5 | Final verification | ~5 | Task 4 |

**Total: ~290 lines new/changed code + ~120 lines of tests**

**Phase 2 unlocks:** Moments 1 (pronoun resolution), 3 (cross-domain follow-up), 5 (time-anchored recall), 8 (coaching recall). These are the queries where the heuristic has no context to enrich — only the LLM can resolve "that thing we discussed" by reading conversation history.
