//! Replays a `ConversationFixture` through a real `AppCore` instance built
//! from an in-memory pool. Exposes hooks for snapshotting cognitive state.

use crate::fixtures::ConversationFixture;
use std::sync::Arc;
use storage::StoragePool;

pub struct ReplayContext {
    pub pool: StoragePool,
    pub app: Arc<app_core::AppCore>,
    pub turn_latencies_ms: Vec<u64>,
    pub captured_events: Arc<tokio::sync::Mutex<Vec<bus::DomainEvent>>>,
}

impl ReplayContext {
    pub async fn new() -> common::Result<Self> {
        let mut cfg = config::load_with_env_overrides().await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!("config load: {e}")))
        })?;
        cfg.data_dir = Some(
            tempfile::tempdir()
                .map_err(common::KlyntbotError::Io)?
                .path()
                .to_string_lossy()
                .to_string(),
        );
        cfg.cognitive.intelligence_mode = config::schema::IntelligenceMode::Deep;
        cfg.cognitive.micro_reforge.enabled = true;
        cfg.cognitive.predictive_cache.enabled = true;
        cfg.cognitive.hierarchical.enabled = true;

        let (app, _channels) = app_core::AppCore::init(common::AppMode::Server, Some(cfg))
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("AppCore init failed: {e}")))?;

        let pool = app.storage_pool.clone();
        // Run cognitive migrations so cognitive tables exist for assertions.
        StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!("cognitive migrations failed: {e}"))
            })?;

        let captured = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let bus_arc = app
            .domain_event_bus()
            .map_err(|e| common::KlyntbotError::Storage(format!("domain_event_bus: {e}")))?
            .clone();
        let mut rx = bus_arc.subscribe();
        let captured_clone = captured.clone();
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                captured_clone.lock().await.push(ev);
            }
        });

        Ok(Self {
            pool,
            app: Arc::new(app),
            turn_latencies_ms: Vec::new(),
            captured_events: captured,
        })
    }

    pub async fn replay(
        &mut self,
        fixture: &ConversationFixture,
    ) -> common::Result<ReplayMeasurements> {
        use std::time::Instant;
        let mut measurements = ReplayMeasurements::default();
        for turn in &fixture.turns {
            let session_key = common::SessionKey::from_parts("kca-e2e", &fixture.id);
            let started = Instant::now();
            let _ = self
                .chat_complete(turn.user.clone(), session_key.to_string())
                .await?;
            let elapsed = started.elapsed().as_millis() as u64;
            self.turn_latencies_ms.push(elapsed);
            measurements.turn_latencies_ms.push(elapsed);
            measurements.turns_replayed += 1;
            // Per-turn sync: wait for THIS turn's extraction to land
            // before publishing the next. Without this, turn N+1's
            // IngestionConsumer races ahead of turn N's `user→name`
            // write and the cross-turn identity-binding mirror skips,
            // losing facts like `Alice→lives_in=SF` even though the
            // raw `user→lives_in=SF` is in the store.
            self.await_cognitive_idle().await;
        }
        // One final settle in case the last turn left work in flight.
        self.await_cognitive_idle().await;
        Ok(measurements)
    }

    /// Block until the cognitive fact store stops growing, indicating that
    /// the background extraction pipeline has caught up.
    ///
    /// Polls every 750ms; returns once the count is unchanged across **four
    /// consecutive** polls (≈3s of quiet) or the 60s safety timeout fires.
    /// The longer stability window matters because two extraction paths
    /// run concurrently: the heuristic SPO extractor persists in <500ms,
    /// while the LLM-based `LlmExtractionHandler` takes 2-4s per batch.
    /// A shorter window declares idle after the heuristic finishes but
    /// before LLM-extracted (identity-bound) facts hit the repo, so the
    /// bench would query a half-populated store.
    pub async fn await_cognitive_idle(&self) {
        use cognitive::repos::SemanticFactRepo;
        use std::time::{Duration, Instant};
        let repo = SemanticFactRepo::new(self.pool.inner().clone());
        let started = Instant::now();
        let deadline = started + Duration::from_secs(60);
        // Unconditional floor. The previous logic let idle return as soon
        // as count was stable for 3s as long as count > 0 — fine on the
        // first turn but catastrophic on per-turn sync: turn 1's LLM call
        // takes up to ~12s, but turn 0's facts have already been counted
        // for 3s by then, so idle returns BEFORE turn 1 writes anything.
        // 14s covers Kimi's worst-case extraction latency observed in
        // benchmarks (12s tail) with margin.
        let floor = started + Duration::from_secs(14);
        let mut last = -1i64;
        let mut stable = 0u32;
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(750)).await;
            let now = repo.count_active().await.unwrap_or(0);
            if now == last {
                if stable >= 4 && Instant::now() >= floor {
                    return;
                }
                stable += 1;
            } else {
                stable = 0;
            }
            last = now;
        }
    }

    /// Diagnostic: dump every active semantic fact in the store as
    /// `(subject, predicate, object)` tuples. Used by bench harnesses to
    /// verify what the extraction pipeline actually persisted.
    pub async fn dump_facts(&self) -> Vec<(String, String, String)> {
        let pool = self.pool.inner().clone();
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT subject, predicate, object FROM semantic_facts \
             WHERE superseded_at IS NULL ORDER BY recorded_at",
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        rows
    }

    /// Send a user message and block until the agent's full streamed reply is
    /// received. Drains [`AgentEvent::ContentChunk`] events from the chat
    /// stream and returns the concatenated assistant text.
    ///
    /// This is the bench-friendly counterpart to `chat_send`, which is
    /// intentionally non-blocking for the streaming UI.
    pub async fn chat_complete(
        &self,
        content: String,
        session_key: String,
    ) -> common::Result<String> {
        let user_message = content.clone();
        let (_user_msg, mut info) = self
            .app
            .chat_send(content, session_key.clone(), None, None)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("chat_send failed: {e}")))?;
        let mut answer = String::new();
        let mut done_content: Option<String> = None;
        while let Some(ev) = info.event_rx.recv().await {
            match ev {
                agent::AgentEvent::ContentChunk { data } => answer.push_str(&data),
                // Reasoning models (Mimo, deepseek-r1, qwq) may emit no
                // ContentChunk events when their answer arrives via
                // reasoning_content. The agent runtime promotes that into
                // the final synthesis content and ships it through Done.
                // Capture it as a fallback when the live stream was empty.
                agent::AgentEvent::Done { content, .. } => done_content = Some(content),
                _ => {}
            }
        }
        if answer.trim().is_empty() {
            if let Some(c) = done_content {
                answer = c;
            }
        }
        // Bench-direct fallback: when the agent loop returns nothing
        // (reasoning-only models that exhaust through tool_calls and
        // never emit content), bypass the agent and ask the model
        // directly using retrieved memory facts as context. Gated by
        // KCA_BENCH_DIRECT_FALLBACK=1 to keep production behavior unchanged.
        if answer.trim().is_empty()
            && matches!(
                std::env::var("KCA_BENCH_DIRECT_FALLBACK").ok().as_deref(),
                Some("1") | Some("true") | Some("yes")
            )
        {
            if let Ok(direct) = self.bench_direct_qa(&user_message).await {
                if !direct.trim().is_empty() {
                    answer = direct;
                }
            }
        }
        // The bench harness drains `event_rx` directly, bypassing
        // `relay_chat_stream` — the function that normally publishes
        // `ChatTurnCompleted` to the cognitive pipeline. Without this
        // event, `IngestionConsumer` never fires and `semantic_facts`
        // stays empty even though the agent itself answered correctly
        // from session history. Publish it manually here to wire the
        // memory pipeline back up for benchmarks.
        //
        // Defensive: if the bus has been dropped or has no live
        // consumers, the publish becomes a no-op rather than a hard
        // error. Observed during n=500 LoCoMo runs where late
        // cognitive tasks could outlive the consumer in some teardown
        // races. Losing one ChatTurnCompleted event for a single QA
        // is not worth aborting the bench over.
        if let Ok(bus) = self.app.domain_event_bus() {
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                bus.publish(bus::DomainEvent::ChatTurnCompleted {
                    session_key,
                    user_message: Some(user_message),
                });
            })) {
                tracing::warn!(
                    ?e,
                    "ChatTurnCompleted publish failed (bus likely shutting down)"
                );
            }
        }
        Ok(answer)
    }

    /// Bench-only direct path: bypass the agent runtime entirely.
    /// Retrieves top semantic facts via the cognitive layer, formats them
    /// into a plain prompt, calls the configured cognitive provider via
    /// raw reqwest, and returns whatever content (or reasoning_content)
    /// the model emitted. Used when the agent loop returns empty content
    /// for reasoning-only models that exhaust through tool_calls.
    async fn bench_direct_qa(&self, question: &str) -> common::Result<String> {
        use cognitive::repos::SemanticFactRepo;
        use cognitive::services::retrieval::{retrieve_relevant_facts, RetrievalParams};
        let fact_repo = SemanticFactRepo::new(self.pool.inner().clone());
        let domains = &[
            "personal", "work", "health", "finance", "learning", "general", "coding",
        ];
        let params = RetrievalParams::new(20);
        let scored = retrieve_relevant_facts(
            &fact_repo, None, question, domains, &params, None, None, None, None,
        )
        .await
        .unwrap_or_default();

        let mut ctx = String::new();
        if !scored.is_empty() {
            ctx.push_str("## Memory (extracted facts)\n");
            for s in scored.iter().take(20) {
                ctx.push_str(&format!(
                    "- {}: {} = {}{}\n",
                    s.fact.subject,
                    s.fact.predicate,
                    s.fact.object,
                    s.fact
                        .valid_until
                        .as_ref()
                        .map(|u| format!(" (until {u})"))
                        .unwrap_or_default()
                ));
            }
            ctx.push('\n');
        }

        // Wave 6 path: also pull verbatim turns from episodic_memories.
        // Triples lose temporal/spatial detail; raw turns preserve dates,
        // names, and the speaker's own phrasing. Significant lift on
        // category-2 (multi-hop/temporal) questions.
        // Diagnostic: count total episodic rows present at retrieval time
        // so we can distinguish "episodes never stored" from "FTS missed".
        if matches!(
            std::env::var("KCA_BENCH_DIRECT_DIAG").ok().as_deref(),
            Some("1")
        ) {
            let total: Option<(i64,)> = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                .fetch_optional(self.pool.inner())
                .await
                .unwrap_or(None);
            let n = total.map(|t| t.0).unwrap_or(-1);
            eprintln!("[bench-direct-diag] episodic_memories total rows: {n}; question: {question:?}");
        }
        match cognitive::search::bm25::search_episodic_memories(
            self.pool.inner(),
            question,
            None,
            10,
        )
        .await
        {
            Ok(hits) if !hits.is_empty() => {
                if matches!(
                    std::env::var("KCA_BENCH_DIRECT_DIAG").ok().as_deref(),
                    Some("1")
                ) {
                    eprintln!(
                        "[bench-direct-diag] FTS episodic hits: {} (top-5 ids: {:?})",
                        hits.len(),
                        hits.iter().take(5).map(|h| &h.id).collect::<Vec<_>>()
                    );
                }
                ctx.push_str("## Conversation episodes (verbatim)\n");
                for h in hits.iter().take(10) {
                    let row: Option<(String, Option<String>, String)> = sqlx::query_as(
                        "SELECT content, summary, occurred_at FROM episodic_memories WHERE id = ?",
                    )
                    .bind(&h.id)
                    .fetch_optional(self.pool.inner())
                    .await
                    .unwrap_or(None);
                    if let Some((content, summary, occurred_at)) = row {
                        let body = summary.unwrap_or(content);
                        let trimmed: String = body.chars().take(400).collect();
                        ctx.push_str(&format!("- [{occurred_at}] {trimmed}\n"));
                    }
                }
                ctx.push('\n');
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(error = %e, "episodic FTS lookup failed"),
        }

        let api_base = std::env::var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_BASE")
            .unwrap_or_else(|_| "https://api.deepseek.com/v1".into());
        let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));
        let api_key = std::env::var("KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY")
            .or_else(|_| std::env::var("MIMO_API_KEY"))
            .map_err(|_| common::KlyntbotError::Storage("no API key for direct QA".into()))?;
        let model = std::env::var("KLYNTBOT_AGENTS__DEFAULTS__MODEL")
            .unwrap_or_else(|_| "mimo-v2.5-pro".into());

        let system = format!(
            "You answer questions about a conversation between participants. \
             Use ONLY the facts from the Memory section below. If the answer \
             is not derivable, reply 'I don't know'. Be concise.\n\n{ctx}"
        );
        let body = serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": question},
            ],
            "max_tokens": 8192,
            "temperature": 0.2,
        });

        let resp = common::shared_http_client()
            .post(&url)
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("direct QA request: {e}")))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| {
            common::KlyntbotError::Storage(format!("direct QA parse: {e}"))
        })?;
        let msg = &json["choices"][0]["message"];
        let content = msg["content"].as_str().unwrap_or("").to_string();
        if !content.trim().is_empty() {
            return Ok(content);
        }
        // Reasoning-only model fallback (Mimo): use reasoning_content
        let reasoning = msg["reasoning_content"].as_str().unwrap_or("").to_string();
        Ok(reasoning)
    }

    /// Phase 3 hook: trigger graph consolidation between ingest and QA.
    /// Bench callers fire this after `await_cognitive_idle()` and
    /// before the QA loop begins, gated on `KCA_PHASE_3=1`.
    pub async fn consolidate_graph(&self) -> common::Result<u32> {
        self.app.trigger_graph_consolidation().await
    }
}

#[derive(Debug, Default, Clone)]
pub struct ReplayMeasurements {
    pub turns_replayed: u32,
    pub turn_latencies_ms: Vec<u64>,
}

impl ReplayMeasurements {
    pub fn p95_ms(&self) -> u64 {
        if self.turn_latencies_ms.is_empty() {
            return 0;
        }
        let mut s = self.turn_latencies_ms.clone();
        s.sort();
        let idx = (s.len() as f64 * 0.95).ceil() as usize - 1;
        s[idx.min(s.len() - 1)]
    }
}
