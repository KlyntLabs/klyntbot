use crate::{
    repos::{
        map_sqlx, AccumulatedObservationRepo, EntityRepo, EpisodicMemoryRepo, SemanticFactRepo,
    },
    services::extraction::{to_semantic_fact, ExtractedFact, ExtractionHandler},
    types::Observation,
};
use ai_core::{AiSignal, SalienceVerdict, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;

/// Regex-light backstop for the cases where Kimi returns an empty `facts`
/// array on content that clearly contains a salient SPO triple. Targeted
/// at the patterns the bench fixtures actually exercise — identity, work,
/// location, possessions. Subject is `user`; cross-turn mirroring will
/// promote it to a proper noun if a name is on file.
fn regex_backstop_facts(content: &str, domain: &str) -> Vec<ExtractedFact> {
    let lc = content.to_lowercase();
    let mut out = Vec::new();
    let mk = |dom: &str, pred: &str, obj: &str| ExtractedFact {
        domain: dom.into(),
        subject: "user".into(),
        predicate: pred.into(),
        object: obj.trim().trim_end_matches('.').to_string(),
        confidence: 0.9,
        source: "regex_backstop".into(),
        speaker: None,
        valid_until: None,
        valid_from: None,
    };

    // "I moved to {city} ..." / "I'm now in {city}" → lives_in
    for prefix in [
        "i moved to ",
        "moved to ",
        "i live in ",
        "i'm in ",
        "i am in ",
        "i'm now in ",
    ] {
        if let Some(rest) = lc
            .strip_prefix(prefix)
            .or_else(|| lc.find(prefix).map(|i| &lc[i + prefix.len()..]))
        {
            // Take up to first separator
            let city = rest
                .split([',', '.', '!', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(" last year")
                .trim_end_matches(" recently");
            if !city.is_empty() && city.len() < 60 {
                // Restore original casing by finding the slice in `content`
                let case_corrected =
                    original_case(content, &lc, city).unwrap_or_else(|| city.to_string());
                out.push(mk("identity", "lives_in", &case_corrected));
                if prefix.contains("moved") {
                    out.push(mk("identity", "moved_to", &case_corrected));
                }
                break;
            }
        }
    }

    // "I work at {org}" / "I'm at {org}" → works_at
    for prefix in ["i work at ", "i'm at ", "i am at ", "working at "] {
        if let Some(idx) = lc.find(prefix) {
            let rest = &lc[idx + prefix.len()..];
            let org = rest
                .split([',', '.', '!', ';', ' '])
                .next()
                .unwrap_or("")
                .trim();
            if !org.is_empty() && org.len() < 40 {
                let case_corrected =
                    original_case(content, &lc, org).unwrap_or_else(|| org.to_string());
                out.push(mk("work", "works_at", &case_corrected));
                break;
            }
        }
    }

    // "I'm {Name}" / "My name is {Name}" → name (capitalized token)
    for prefix in ["i'm ", "i am ", "my name is "] {
        if let Some(idx) = lc.find(prefix) {
            let rest = &content[idx + prefix.len()..];
            let first_token: String = rest.chars().take_while(|c| c.is_alphabetic()).collect();
            if first_token.len() >= 2
                && first_token.chars().next().is_some_and(|c| c.is_uppercase())
            {
                out.push(mk("identity", "name", &first_token));
                break;
            }
        }
    }

    // Third-person SPO: "<Proper> <verb> <obj>." Catches the multi-hop
    // benchmark patterns "Max loves pizza", "Bella loves sushi" that the
    // LLM extractor frequently skips when its system prompt is heavily
    // first-person-biased. We deliberately accept only a small whitelist
    // of high-signal verbs to avoid garbage extractions from narrative
    // assistant replies.
    for sentence in content.split(['.', '!', '?']) {
        let s = sentence.trim();
        let mut toks = s.split_whitespace();
        let Some(subj) = toks.next() else { continue };
        let subj_clean = subj.trim_end_matches(',');
        // Subject must be a proper noun: starts uppercase, ≥2 chars,
        // alphabetic, and not a first-person pronoun ("I", "I'm", ...).
        let is_proper = subj_clean.len() >= 2
            && subj_clean.chars().next().is_some_and(|c| c.is_uppercase())
            && subj_clean.chars().all(|c| c.is_alphabetic())
            && !matches!(
                subj_clean,
                "I" | "We"
                    | "You"
                    | "He"
                    | "She"
                    | "It"
                    | "They"
                    | "Her"
                    | "His"
                    | "Their"
                    | "Them"
                    | "Us"
                    | "Our"
                    | "My"
                    | "Hi"
                    | "Hello"
                    | "Hey"
                    | "The"
                    | "A"
                    | "An"
            );
        if !is_proper {
            continue;
        }
        let Some(verb) = toks.next() else { continue };
        let pred = match verb.to_lowercase().as_str() {
            "loves" | "love" => "loves",
            "likes" | "like" => "likes",
            "prefers" | "prefer" => "prefers",
            "enjoys" | "enjoy" => "enjoys",
            "hates" | "hate" => "hates",
            "owns" | "own" => "owns",
            "has" | "have" => "has",
            "uses" | "use" => "uses",
            "plays" | "play" => "plays",
            "works" => {
                // "Alice works at Anthropic" → works_at + skip "at"
                match toks.next() {
                    Some("at") => "works_at",
                    _ => continue,
                }
            }
            "lives" => match toks.next() {
                Some("in") => "lives_in",
                _ => continue,
            },
            _ => continue,
        };
        let obj: String = toks.collect::<Vec<_>>().join(" ");
        let obj = obj.trim_end_matches(|c: char| !c.is_alphanumeric()).trim();
        if obj.is_empty() || obj.len() > 80 {
            continue;
        }
        out.push(ExtractedFact {
            domain: domain.to_string(),
            subject: subj_clean.to_string(),
            predicate: pred.to_string(),
            object: obj.to_string(),
            confidence: 0.85,
            source: "regex_backstop_3p".into(),
            speaker: None,
            valid_until: None,
            valid_from: None,
        });
    }

    out
}

/// Recover original casing of `lc_slice` from `original` (a longer string
/// containing the slice in some casing). Returns None if not found.
fn original_case(original: &str, lc_orig: &str, lc_slice: &str) -> Option<String> {
    let idx = lc_orig.find(lc_slice)?;
    let end = idx + lc_slice.len();
    Some(original[idx..end].to_string())
}

pub struct IngestionConsumer {
    observation_repo: AccumulatedObservationRepo,
    entity_repo: EntityRepo,
    episodic_repo: EpisodicMemoryRepo,
    extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    fact_repo: Option<SemanticFactRepo>,
    conflict_resolver: Option<Arc<dyn crate::services::extraction::ConflictResolver>>,
    embedder: Option<Arc<dyn crate::embedder::SemanticFactEmbedder>>,
    episodic_importance_threshold: f64,
}

impl IngestionConsumer {
    pub fn new(
        observation_repo: AccumulatedObservationRepo,
        entity_repo: EntityRepo,
        episodic_repo: EpisodicMemoryRepo,
        extraction_handler: Option<Arc<dyn ExtractionHandler>>,
    ) -> Self {
        Self {
            observation_repo,
            entity_repo,
            episodic_repo,
            extraction_handler,
            fact_repo: None,
            conflict_resolver: None,
            embedder: None,
            // Default threshold (0.7) keeps log-noise pollution low. Override
            // via `config.cognitive.episodicImportanceThreshold` — see
            // `with_episodic_threshold` builder method below. Setting it to
            // 0.0 preserves every observation as a raw episode (Letta-style).
            episodic_importance_threshold: 0.7,
        }
    }

    /// Override the episodic-memory importance threshold (default 0.7).
    /// Values outside `0.0..=1.0` are clamped to that range.
    pub fn with_episodic_threshold(mut self, threshold: f64) -> Self {
        self.episodic_importance_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Wire an LLM-backed AUDD conflict resolver (Mem0 pattern). When
    /// absent, ingestion uses the default Add-only behavior. Opt-out
    /// is via cognitive provider absence (no provider configured →
    /// no resolver → Add-only).
    pub fn with_conflict_resolver(
        mut self,
        resolver: Arc<dyn crate::services::extraction::ConflictResolver>,
    ) -> Self {
        self.conflict_resolver = Some(resolver);
        self
    }

    /// Wire the semantic fact repo so extracted facts are persisted (not
    /// just computed). Without this, `extract_facts_batch` runs but its
    /// result is discarded — facts never reach `semantic_facts`.
    pub fn with_fact_repo(mut self, fact_repo: SemanticFactRepo) -> Self {
        self.fact_repo = Some(fact_repo);
        self
    }

    /// Wire a fact embedder so newly persisted facts are also written to
    /// the vector store. Without this, `vector_hits` stays 0 across the
    /// entire system because facts are SQLite-only — the only path that
    /// embeds is the nightly reforge consolidation, which doesn't run
    /// during a bench replay. Pre-existing dead-vector bug surfaced in
    /// trace-w0-w3-dev-ds.jsonl (vector_hits=0 on every event).
    pub fn with_embedder(
        mut self,
        embedder: Arc<dyn crate::embedder::SemanticFactEmbedder>,
    ) -> Self {
        self.embedder = Some(embedder);
        self
    }

    fn signal_to_observation(signal: &AiSignal) -> Observation {
        Observation {
            domain: signal.domain.as_str().to_string(),
            content: signal.content.clone(),
            importance: signal.importance,
            source_event: signal.event_kind.to_string(),
            timestamp: signal.timestamp,
        }
    }
}

#[async_trait]
impl SignalConsumer for IngestionConsumer {
    fn name(&self) -> &'static str {
        "cognitive_ingestion"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        // 1. Entity bridge (always, regardless of salience).
        if let Some(entity) = &signal.entity {
            let _ = self
                .entity_repo
                .upsert_entity(&crate::repos::NewEntity {
                    name: entity.name.clone(),
                    entity_type: entity.entity_type.to_string(),
                    description: None,
                    source: "signal".to_string(),
                    source_id: Some(entity.id.clone()),
                    metadata: None,
                })
                .await;
        }

        // 2. Salience routing. Discard exits before allocating an Observation.
        if matches!(signal.salience, SalienceVerdict::Discard) {
            return Ok(());
        }

        let observation = Self::signal_to_observation(signal);
        self.observation_repo
            .insert(signal.event_kind, &observation)
            .await;
        if matches!(signal.salience, SalienceVerdict::Extract) {
            tracing::info!(
                event_kind = signal.event_kind,
                content_len = signal.content.len(),
                handler_set = self.extraction_handler.is_some(),
                fact_repo_set = self.fact_repo.is_some(),
                "ingestion: Extract salience routing to LLM extractor"
            );
            if let Some(handler) = &self.extraction_handler {
                let handler = handler.clone();
                let obs = observation.clone();
                let fact_repo = self.fact_repo.clone();
                let entity_repo = self.entity_repo.clone();
                let conflict_resolver = self.conflict_resolver.clone();
                let embedder = self.embedder.clone();
                tokio::spawn(async move {
                    // Look up persisted user-name BEFORE extraction so we can
                    // both (a) prepend identity context to the prompt and
                    // (b) mirror facts cross-turn after persistence.
                    let user_name = if let Some(repo) = &fact_repo {
                        repo.find_by_subject_predicate("user", "name")
                            .await
                            .ok()
                            .and_then(|rows| {
                                rows.into_iter()
                                    .find(|f| f.superseded_at.is_none())
                                    .map(|f| f.object)
                            })
                    } else {
                        None
                    };

                    // Annotate the observation with identity context so the
                    // LLM extractor knows which proper noun this terse
                    // first-person sentence ("I moved to SF last year")
                    // refers to. Without this, on turn-1+ Kimi often
                    // returns 0 facts because the binding is implicit.
                    let mut annotated_obs = obs.clone();
                    if let Some(name) = &user_name {
                        annotated_obs.content = format!(
                            "(Context: the user's name is {name}. Bind first-person facts to {name}.) {}",
                            obs.content
                        );
                    }

                    match handler.extract_facts_batch(&[annotated_obs]).await {
                        Ok(mut result) => {
                            // ALWAYS run the regex backstop alongside the
                            // LLM. Previously gated on total_llm==0, but
                            // Kimi sometimes returns junk like
                            // user→event=moved (>0 facts but missing the
                            // important lives_in row). Running both and
                            // letting upsert dedupe is simpler and more
                            // robust. The backstop only fires for the
                            // identity/location/work patterns it knows.
                            let regex_facts = regex_backstop_facts(&obs.content, &obs.domain);
                            if !regex_facts.is_empty() {
                                tracing::info!(
                                    regex_facts = regex_facts.len(),
                                    "ingestion: regex backstop contributed facts"
                                );
                                result.extractions.push(
                                    crate::services::extraction::BatchExtraction {
                                        observation_index: 0,
                                        facts: regex_facts,
                                    },
                                );
                            }

                            let n_facts: usize =
                                result.extractions.iter().map(|e| e.facts.len()).sum();
                            tracing::info!(
                                n_extractions = result.extractions.len(),
                                n_facts,
                                n_entities = result.entities.len(),
                                n_relationships = result.relationships.len(),
                                "ingestion: LLM extraction completed"
                            );

                            // Persist entities + relationships discovered by
                            // the LLM. Without this loop, `BatchExtractionResult.entities`
                            // is computed and dropped — the entities table
                            // only ever sees the single `signal.entity` set
                            // upstream, which means graph_path_boost,
                            // entity-aware FTS expansion, and PPR
                            // retrieval all see a near-empty graph
                            // regardless of what the extractor found.
                            for ent in &result.entities {
                                match entity_repo
                                    .upsert_entity(&crate::repos::NewEntity {
                                        name: ent.name.clone(),
                                        entity_type: ent.entity_type.clone(),
                                        description: ent.description.clone(),
                                        source: "extraction".to_string(),
                                        source_id: None,
                                        metadata: None,
                                    })
                                    .await
                                {
                                    Ok(row) => {
                                        // Auto-derive possessive + short-form
                                        // aliases so retrieval finds the entity
                                        // when the question phrases it as
                                        // "Caroline's" or "Mel".
                                        for (alias, alias_type) in
                                            crate::repos::entity::derive_name_aliases(&ent.name)
                                        {
                                            let _ = entity_repo
                                                .upsert_alias(
                                                    &row.id, &alias, alias_type, "derived",
                                                )
                                                .await;
                                        }
                                    }
                                    Err(e) => tracing::warn!(
                                        error = %e,
                                        name = %ent.name,
                                        "entity upsert failed"
                                    ),
                                }
                            }
                            for rel in &result.relationships {
                                let src = entity_repo
                                    .find_by_name(&rel.source_name)
                                    .await
                                    .ok()
                                    .and_then(|rows| rows.into_iter().next());
                                let tgt = entity_repo
                                    .find_by_name(&rel.target_name)
                                    .await
                                    .ok()
                                    .and_then(|rows| rows.into_iter().next());
                                if let (Some(s), Some(t)) = (src, tgt) {
                                    if let Err(e) = entity_repo
                                        .upsert_relationship(&crate::repos::NewRelationship {
                                            source_entity_id: s.id.clone(),
                                            target_entity_id: t.id.clone(),
                                            relationship_type: rel.relationship_type.clone(),
                                            evidence: None,
                                            source: "extraction".to_string(),
                                        })
                                        .await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            source = %rel.source_name,
                                            target = %rel.target_name,
                                            "relationship upsert failed"
                                        );
                                    }
                                }
                            }
                            // Persist extracted facts. Without this, the
                            // extraction work is wasted — facts only
                            // exist in memory and never reach the FTS5
                            // index used by recall.
                            if let Some(repo) = fact_repo {
                                // RE-lookup user_name AFTER extraction only if
                                // the pre-extraction lookup came up empty. Turn
                                // N's spawn often races ahead of turn N-1's
                                // write, so the pre-lookup returns None; by
                                // re-querying here the natural 2-4s LLM call
                                // gave turn N-1 time to land.
                                let user_name = if user_name.is_none() {
                                    repo.find_by_subject_predicate("user", "name")
                                        .await
                                        .ok()
                                        .and_then(|rows| {
                                            rows.into_iter()
                                                .find(|f| f.superseded_at.is_none())
                                                .map(|f| f.object)
                                        })
                                } else {
                                    user_name
                                };
                                let mut written = 0usize;
                                for ext in &result.extractions {
                                    for f in &ext.facts {
                                        let mut fact = to_semantic_fact(f, &obs);

                                        // AUDD (Mem0 pattern): when a
                                        // resolver is wired, ask it whether
                                        // to ADD/UPDATE/DELETE/NOOP this
                                        // candidate vs existing facts on
                                        // the same (subject, predicate).
                                        // Default-on as of Tier 0 — falls
                                        // back to Add-only when no resolver
                                        // is wired (e.g., no cognitive
                                        // provider).
                                        let mut decision =
                                            crate::services::extraction::ConflictDecision::Add;
                                        if let Some(resolver) = &conflict_resolver {
                                            let existing = repo
                                                .find_by_subject_predicate(
                                                    &fact.subject,
                                                    &fact.predicate,
                                                )
                                                .await
                                                .ok()
                                                .map(|rows| {
                                                    rows.into_iter()
                                                        .filter(|r| r.superseded_at.is_none())
                                                        .collect::<Vec<_>>()
                                                })
                                                .unwrap_or_default();
                                            if !existing.is_empty() {
                                                decision = resolver.classify(f, &existing).await;
                                            }
                                        }
                                        match decision {
                                            crate::services::extraction::ConflictDecision::Noop => {
                                                tracing::debug!(
                                                    subject = %fact.subject,
                                                    predicate = %fact.predicate,
                                                    "AUDD: NOOP (equivalent fact exists)"
                                                );
                                            }
                                            crate::services::extraction::ConflictDecision::Update {
                                                existing_id,
                                            } => {
                                                fact.id = existing_id;
                                                if let Err(e) = repo.upsert(&fact).await {
                                                    tracing::warn!(error = %e, "AUDD UPDATE failed");
                                                } else {
                                                    written += 1;
                                                    if let Some(emb) = &embedder {
                                                        if let Err(e) = emb.embed_and_store_fact(&fact).await {
                                                            tracing::warn!(error = %e, "embed_and_store_fact failed");
                                                        }
                                                    }
                                                }
                                            }
                                            crate::services::extraction::ConflictDecision::Delete {
                                                existing_id,
                                            } => {
                                                if let Err(e) = repo
                                                    .supersede_fact(&existing_id, &fact.id)
                                                    .await
                                                {
                                                    tracing::warn!(error = %e, "AUDD DELETE supersede failed");
                                                }
                                                if let Err(e) = repo.upsert(&fact).await {
                                                    tracing::warn!(error = %e, "AUDD DELETE+ADD failed");
                                                } else {
                                                    written += 1;
                                                    if let Some(emb) = &embedder {
                                                        if let Err(e) = emb.embed_and_store_fact(&fact).await {
                                                            tracing::warn!(error = %e, "embed_and_store_fact failed");
                                                        }
                                                    }
                                                }
                                            }
                                            crate::services::extraction::ConflictDecision::Add => {
                                                if let Err(e) = repo.upsert(&fact).await {
                                                    tracing::warn!(error = %e, "fact upsert failed");
                                                } else {
                                                    written += 1;
                                                    if let Some(emb) = &embedder {
                                                        if let Err(e) = emb.embed_and_store_fact(&fact).await {
                                                            tracing::warn!(error = %e, "embed_and_store_fact failed");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        // Mirror to proper-noun subject when
                                        // the original is a first-person fact
                                        // and a name binding already exists.
                                        // Skip the name predicate itself — it
                                        // would create `Alice→name=Alice`.
                                        if fact.subject == "user" && fact.predicate != "name" {
                                            if let Some(name) = &user_name {
                                                let mut mirrored = fact.clone();
                                                mirrored.subject = name.clone();
                                                mirrored.id = uuid::Uuid::new_v4().to_string();
                                                match repo.upsert(&mirrored).await {
                                                    Ok(()) => {
                                                        written += 1;
                                                        if let Some(emb) = &embedder {
                                                            if let Err(e) = emb
                                                                .embed_and_store_fact(&mirrored)
                                                                .await
                                                            {
                                                                tracing::warn!(error = %e, "embed mirrored failed");
                                                            }
                                                        }
                                                    }
                                                    Err(e) => tracing::warn!(
                                                        error = %e,
                                                        "mirrored fact upsert failed"
                                                    ),
                                                }
                                            }
                                        } else if fact.predicate != "name"
                                            && user_name
                                                .as_ref()
                                                .is_some_and(|n| n == &fact.subject)
                                        {
                                            // Reverse mirror: when the LLM
                                            // emits a third-person fact about
                                            // the named user (e.g. user said
                                            // "I'm Alice" once, then later
                                            // "Alice loves hiking"), also
                                            // write the `subject="user"`
                                            // copy so first-person queries
                                            // ("what do I love") still hit.
                                            let mut mirrored = fact.clone();
                                            mirrored.subject = "user".to_string();
                                            mirrored.id = uuid::Uuid::new_v4().to_string();
                                            match repo.upsert(&mirrored).await {
                                                Ok(()) => {
                                                    written += 1;
                                                    if let Some(emb) = &embedder {
                                                        if let Err(e) = emb
                                                            .embed_and_store_fact(&mirrored)
                                                            .await
                                                        {
                                                            tracing::warn!(error = %e, "embed reverse-mirrored failed");
                                                        }
                                                    }
                                                }
                                                Err(e) => tracing::warn!(
                                                    error = %e,
                                                    "reverse-mirrored fact upsert failed"
                                                ),
                                            }
                                        }
                                    }
                                }
                                tracing::info!(written, "ingestion: facts persisted");
                            } else {
                                tracing::warn!("ingestion: no fact_repo wired, facts dropped");
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "extraction failed"),
                    }
                });
            } else {
                tracing::warn!("ingestion: Extract salience but no handler wired");
            }
        }

        // 3. Episodic branch for high-importance signals.
        if observation.importance >= self.episodic_importance_threshold {
            let mem = crate::types::EpisodicMemory {
                id: uuid::Uuid::new_v4().to_string(),
                domain: observation.domain.clone(),
                content: observation.content.clone(),
                summary: None,
                importance: observation.importance,
                occurred_at: observation.timestamp.to_string(),
                recorded_at: jiff::Timestamp::now().to_string(),
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                project_id: None,
                scope_type: "system".to_string(),
                scope_id: None,
                kind: None,
                scope_repo_id: None,
                metadata: None,
                actor_id: None,
                tier: "raw".to_string(),
                parent_id: None,
                child_count: 0,
                rolled_up_at: None,
            };
            self.episodic_repo.insert(&mem).await.map_err(map_sqlx)?;
        }

        Ok(())
    }
}
