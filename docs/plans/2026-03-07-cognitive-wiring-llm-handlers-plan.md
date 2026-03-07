# Cognitive Wiring & LLM Handler Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the cognitive architecture's missing connections and implement LLM-backed handlers so the system learns from user activity and coaches proactively.

**Architecture:** Dedicated cognitive provider (cheap model) for background tasks. Four LLM handlers with heuristic fallbacks. New `CoachingService` subscribes to DomainEventBus. Weekly reflection scheduled via CronService.

**Tech Stack:** Rust, async-trait, serde_json, tokio broadcast channels, providers crate (ResponseFormat::JsonSchema)

**Design doc:** `docs/plans/2026-03-07-cognitive-wiring-llm-handlers-design.md`

---

### Task 1: Add CognitiveConfig to config crate

**Files:**
- Create: `crates/config/src/schema/cognitive.rs`
- Modify: `crates/config/src/schema/mod.rs` (add module + re-export)
- Modify: `crates/config/src/schema/core.rs` (add field to Config struct)

**Step 1: Create the config struct**

Create `crates/config/src/schema/cognitive.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Configuration for background cognitive tasks (extraction, consolidation,
/// reflection, coaching reasoning).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveConfig {
    /// Model for cognitive LLM calls. Falls back to agents.defaults.model if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider name override. Falls back to agents.defaults.provider if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Temperature for cognitive calls (default: 0.2 — low creativity, high consistency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Max tokens per cognitive call (default: 1024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Max tokens for reflection calls (default: 2048).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_max_tokens: Option<u32>,

    /// Cron expression for weekly reflection (default: "0 9 * * 1" — Monday 9am).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_schedule: Option<String>,
}
```

**Step 2: Register in schema module**

In `crates/config/src/schema/mod.rs`, add alongside existing module declarations:

```rust
mod cognitive;
```

And in the `pub use` block:

```rust
pub use self::cognitive::*;
```

**Step 3: Add field to Config struct**

In `crates/config/src/schema/core.rs`, add to the `Config` struct (after the `mcp` field):

```rust
    /// Cognitive memory & coaching configuration.
    #[serde(default)]
    pub cognitive: CognitiveConfig,
```

**Step 4: Verify**

Run: `cargo build -p config`
Expected: compiles cleanly.

Run: `cargo nextest run -p config`
Expected: all existing tests pass (default config still deserializes correctly since new field has `#[serde(default)]`).

---

### Task 2: Add create_cognitive_provider() to providers crate

**Files:**
- Modify: `crates/providers/src/lib.rs` (add function)
- Test: inline test in same file

**Step 1: Write the test**

Add to existing `#[cfg(test)] mod tests` in `crates/providers/src/lib.rs`:

```rust
#[test]
fn test_create_cognitive_provider_returns_none_when_no_keys() {
    let config = config::Config::default();
    let result = create_cognitive_provider(&config);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_cognitive_chat_params_defaults() {
    let config = config::Config::default();
    let params = cognitive_chat_params(&config, 1024);
    assert_eq!(params.temperature, Some(0.2));
    assert_eq!(params.max_tokens, Some(1024));
}

#[test]
fn test_cognitive_chat_params_with_overrides() {
    let mut config = config::Config::default();
    config.cognitive.temperature = Some(0.5);
    config.cognitive.max_tokens = Some(2048);
    let params = cognitive_chat_params(&config, 1024);
    assert_eq!(params.temperature, Some(0.5));
    assert_eq!(params.max_tokens, Some(2048));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p providers -E 'test(cognitive)'`
Expected: FAIL — functions don't exist yet.

**Step 3: Implement**

Add to `crates/providers/src/lib.rs`:

```rust
/// Create a provider for background cognitive tasks.
///
/// Uses `config.cognitive.model` and `config.cognitive.provider` if set,
/// otherwise falls back to the main agent provider. Returns `None` if
/// no provider can be created (no API keys configured).
pub fn create_cognitive_provider(config: &Config) -> Result<Option<DynProvider>> {
    // If cognitive-specific model/provider is set, build a temporary config override
    if config.cognitive.model.is_some() || config.cognitive.provider.is_some() {
        let mut cognitive_config = config.clone();
        if let Some(ref model) = config.cognitive.model {
            cognitive_config.agents.defaults.model = model.clone();
        }
        if let Some(ref provider) = config.cognitive.provider {
            cognitive_config.agents.defaults.provider = Some(provider.clone());
        }
        match create_provider(&cognitive_config) {
            Ok((provider, _model)) => return Ok(Some(provider)),
            Err(_) => {} // Fall through to try main provider
        }
    }

    // Fall back to main agent provider
    match create_provider(config) {
        Ok((provider, _model)) => Ok(Some(provider)),
        Err(_) => Ok(None), // No provider available — handlers will use heuristics
    }
}

/// Build `ChatParams` for cognitive LLM calls.
pub fn cognitive_chat_params(config: &Config, default_max_tokens: u32) -> ChatParams {
    let model = config
        .cognitive
        .model
        .as_deref()
        .unwrap_or(&config.agents.defaults.model);
    let temperature = config.cognitive.temperature.unwrap_or(0.2);
    let max_tokens = config.cognitive.max_tokens.unwrap_or(default_max_tokens);

    ChatParams::new(model)
        .with_temperature(temperature)
        .with_max_tokens(max_tokens)
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p providers -E 'test(cognitive)'`
Expected: PASS.

---

### Task 3: HeuristicReflectionHandler

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs` (add handler)

**Step 1: Write the test**

Add to existing `#[cfg(test)] mod tests` in `crates/agent/src/cognitive_handlers.rs`:

```rust
#[tokio::test]
async fn test_heuristic_reflection_returns_summary() {
    use cognitive::reflection::{ReflectionHandler, ReflectionInput};
    use cognitive::types::{EpisodicMemory, ProceduralRule, UserModel};

    let handler = HeuristicReflectionHandler;
    let input = ReflectionInput {
        episodic_memories: vec![EpisodicMemory {
            id: "e1".into(),
            domain: "productivity".into(),
            content: "Had a productive morning".into(),
            summary: Some("Productive morning".into()),
            importance: 0.7,
            occurred_at: "2026-03-01T10:00:00".into(),
            recorded_at: "2026-03-01T10:00:00".into(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
        }],
        user_model: UserModel::default(),
        procedural_rules: vec![],
        period_start: "2026-03-01T00:00:00".into(),
        period_end: "2026-03-07T23:59:59".into(),
    };

    let output = handler.reflect(&input).await.unwrap();
    assert!(!output.summary.is_empty());
    assert!(output.fact_updates.is_empty()); // Heuristic doesn't generate updates
    assert!(output.rule_updates.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(heuristic_reflection)'`
Expected: FAIL.

**Step 3: Implement**

Add to `crates/agent/src/cognitive_handlers.rs`:

```rust
use cognitive::reflection::{ReflectionHandler, ReflectionInput, ReflectionOutput};

/// Heuristic reflection — generates a statistical summary without LLM.
/// Returns empty fact/rule updates but provides a useful summary.
pub struct HeuristicReflectionHandler;

#[async_trait]
impl ReflectionHandler for HeuristicReflectionHandler {
    async fn reflect(&self, input: &ReflectionInput) -> common::Result<ReflectionOutput> {
        let mem_count = input.episodic_memories.len();
        let rule_count = input.procedural_rules.len();
        let domains: std::collections::HashSet<&str> = input
            .user_model
            .domains
            .keys()
            .map(|k| k.as_str())
            .collect();

        let summary = format!(
            "Heuristic reflection ({} to {}): {} episodic memories, {} active rules, {} domains tracked. \
             No LLM available for cross-domain synthesis.",
            input.period_start, input.period_end, mem_count, rule_count, domains.len()
        );

        Ok(ReflectionOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            summary,
        })
    }
}
```

**Step 4: Run test**

Run: `cargo nextest run -p agent -E 'test(heuristic_reflection)'`
Expected: PASS.

---

### Task 4: LlmExtractionHandler

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_llm_extraction_parses_json_response() {
    let mock = Arc::new(MockProvider::new(LlmResponse {
        content: Some(r#"{"facts":[{"domain":"energy","subject":"user","predicate":"peak_hours","object":"10am-12pm","confidence":0.85,"source":"observed"}]}"#.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
        usage: Default::default(),
        reasoning_content: None,
    }));
    let params = ChatParams::new("test-model").with_temperature(0.2).with_max_tokens(1024);
    let handler = LlmExtractionHandler::new(mock, params);

    let obs = Observation {
        domain: "productivity".into(),
        content: "User is most productive between 10am and 12pm".into(),
        importance: 0.8,
        source_event: "ProductivityScoreComputed".into(),
        timestamp: Utc::now(),
    };
    let facts = handler.extract_facts(&obs).await.unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].domain, "energy");
    assert_eq!(facts[0].predicate, "peak_hours");
    assert_eq!(facts[0].object, "10am-12pm");
}

#[tokio::test]
async fn test_llm_extraction_falls_back_on_error() {
    let mock = Arc::new(MockProvider::new_error("LLM unavailable"));
    let params = ChatParams::new("test-model");
    let handler = LlmExtractionHandler::new(mock, params);

    let obs = Observation {
        domain: "productivity".into(),
        content: "User stated: I like mornings".into(),
        importance: 1.0,
        source_event: "UserStatedFact".into(),
        timestamp: Utc::now(),
    };
    // Should fall back to heuristic, not error
    let facts = handler.extract_facts(&obs).await.unwrap();
    assert!(!facts.is_empty()); // Heuristic handles UserStatedFact
}
```

Note: `MockProvider` and `MockProvider::new_error` will need to be added to the test module. See the existing `MockProvider` pattern in `crates/agent/src/agent_loop/refactor_tests.rs`.

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(llm_extraction)'`
Expected: FAIL.

**Step 3: Implement**

```rust
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use serde_json::json;

/// JSON schema for extraction output.
fn extraction_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "facts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "domain": { "type": "string" },
                        "subject": { "type": "string" },
                        "predicate": { "type": "string" },
                        "object": { "type": "string" },
                        "confidence": { "type": "number" },
                        "source": { "type": "string", "enum": ["observed", "inferred", "user_stated", "reflected"] }
                    },
                    "required": ["domain", "subject", "predicate", "object", "confidence", "source"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["facts"],
        "additionalProperties": false
    })
}

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a semantic memory extraction agent. Given an observation about a user, extract structured facts as subject-predicate-object triples.

Domains: identity, energy, work, finance, learning, preferences
Subjects: usually "user", or "project:<name>", "task:<id>"
Predicates: descriptive relationship (e.g., "peak_hours", "spending_pattern", "break_pattern", "estimation_accuracy")
Object: the value (e.g., "10am-12pm", "food delivery spikes during crunch")

Rules:
- Only extract facts clearly supported by the observation
- Set confidence based on how certain the observation is (user-stated = 1.0, inferred = 0.5-0.8)
- Use source "user_stated" for explicit user statements, "observed" for behavioral data, "inferred" for patterns
- Return empty facts array if nothing meaningful can be extracted
- Be specific in predicates — "peak_hours" not "time"
- Be concise in objects — "10am-12pm" not "The user tends to be most productive between 10am and 12pm""#;

/// LLM-backed fact extraction with heuristic fallback.
pub struct LlmExtractionHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicExtractionHandler,
}

impl LlmExtractionHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonSchema {
                name: "ExtractionResult".into(),
                schema: extraction_schema(),
            }),
            fallback: HeuristicExtractionHandler,
        }
    }
}

#[derive(serde::Deserialize)]
struct ExtractionResult {
    facts: Vec<ExtractedFactJson>,
}

#[derive(serde::Deserialize)]
struct ExtractedFactJson {
    domain: String,
    subject: String,
    predicate: String,
    object: String,
    confidence: f64,
    source: String,
}

#[async_trait]
impl ExtractionHandler for LlmExtractionHandler {
    async fn extract_facts(&self, observation: &Observation) -> common::Result<Vec<ExtractedFact>> {
        let user_msg = format!(
            "Domain: {}\nSource: {}\nImportance: {:.1}\n\nObservation:\n{}",
            observation.domain, observation.source_event, observation.importance, observation.content
        );

        let messages = vec![
            Message::system(EXTRACTION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<ExtractionResult>(&content) {
                    Ok(result) => Ok(result
                        .facts
                        .into_iter()
                        .map(|f| ExtractedFact {
                            domain: f.domain,
                            subject: f.subject,
                            predicate: f.predicate,
                            object: f.object,
                            confidence: f.confidence,
                            source: f.source,
                        })
                        .collect()),
                    Err(e) => {
                        tracing::warn!("LLM extraction JSON parse failed: {e}, falling back to heuristic");
                        self.fallback.extract_facts(observation).await
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM extraction call failed: {e}, falling back to heuristic");
                self.fallback.extract_facts(observation).await
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(llm_extraction)'`
Expected: PASS.

---

### Task 5: LlmConsolidationHandler

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_llm_consolidation_parses_update() {
    let mock = Arc::new(MockProvider::new(LlmResponse {
        content: Some(r#"{"action":"update","target_id":"old-1","reasoning":"More specific time range","confidence":0.9}"#.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
        usage: Default::default(),
        reasoning_content: None,
    }));
    let params = ChatParams::new("test-model");
    let handler = LlmConsolidationHandler::new(mock, params);

    let candidate = test_fact("new-1", "peak_hours", "9am-11am");
    let existing = vec![test_fact("old-1", "peak_hours", "10am-12pm")];
    let op = handler.decide(&candidate, &existing).await.unwrap();
    assert!(matches!(op, MemoryOp::Update { ref id, ref old_id } if id == "new-1" && old_id == "old-1"));
}

#[tokio::test]
async fn test_llm_consolidation_parses_noop() {
    let mock = Arc::new(MockProvider::new(LlmResponse {
        content: Some(r#"{"action":"noop","target_id":null,"reasoning":"Already known","confidence":1.0}"#.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
        usage: Default::default(),
        reasoning_content: None,
    }));
    let params = ChatParams::new("test-model");
    let handler = LlmConsolidationHandler::new(mock, params);

    let candidate = test_fact("new-1", "peak_hours", "10am-12pm");
    let existing = vec![test_fact("old-1", "peak_hours", "10am-12pm")];
    let op = handler.decide(&candidate, &existing).await.unwrap();
    assert_eq!(op, MemoryOp::Noop);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(llm_consolidation)'`
Expected: FAIL.

**Step 3: Implement**

```rust
fn consolidation_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "action": { "type": "string", "enum": ["add", "update", "delete", "noop"] },
            "target_id": { "type": ["string", "null"] },
            "reasoning": { "type": "string" },
            "confidence": { "type": "number" }
        },
        "required": ["action", "reasoning", "confidence"],
        "additionalProperties": false
    })
}

const CONSOLIDATION_SYSTEM_PROMPT: &str = r#"You are a semantic memory consolidation agent. Given a candidate fact and existing similar facts, decide the correct operation:

- ADD: The candidate is genuinely new information, no existing fact covers it.
- UPDATE: The candidate refines or corrects an existing fact. Provide the target_id of the fact to supersede.
- DELETE: The candidate contradicts an existing fact and the existing fact should be marked superseded. Provide the target_id to delete.
- NOOP: The candidate is already known — an existing fact already captures this information.

Always prefer NOOP over ADD if the information is essentially the same.
Always prefer UPDATE over DELETE+ADD when the meaning is similar but the value changed."#;

/// LLM-backed consolidation with heuristic fallback.
pub struct LlmConsolidationHandler {
    provider: DynProvider,
    params: ChatParams,
    fallback: HeuristicConsolidationHandler,
}

impl LlmConsolidationHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonSchema {
                name: "ConsolidationDecision".into(),
                schema: consolidation_schema(),
            }),
            fallback: HeuristicConsolidationHandler,
        }
    }
}

#[derive(serde::Deserialize)]
struct ConsolidationDecisionJson {
    action: String,
    target_id: Option<String>,
    #[allow(dead_code)]
    reasoning: String,
    #[allow(dead_code)]
    confidence: f64,
}

#[async_trait]
impl ConsolidationHandler for LlmConsolidationHandler {
    async fn decide(
        &self,
        candidate: &SemanticFact,
        existing: &[SemanticFact],
    ) -> common::Result<MemoryOp> {
        let existing_json: Vec<serde_json::Value> = existing
            .iter()
            .map(|f| json!({
                "id": f.id,
                "subject": f.subject,
                "predicate": f.predicate,
                "object": f.object,
                "confidence": f.confidence,
                "source": f.source,
            }))
            .collect();

        let user_msg = format!(
            "Candidate fact:\n  subject: {}\n  predicate: {}\n  object: {}\n  confidence: {}\n\nExisting facts:\n{}",
            candidate.subject, candidate.predicate, candidate.object, candidate.confidence,
            serde_json::to_string_pretty(&existing_json).unwrap_or_default()
        );

        let messages = vec![
            Message::system(CONSOLIDATION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<ConsolidationDecisionJson>(&content) {
                    Ok(decision) => {
                        match decision.action.as_str() {
                            "add" => Ok(MemoryOp::Add { id: candidate.id.clone() }),
                            "update" => {
                                let old_id = decision.target_id.unwrap_or_else(|| {
                                    existing.first().map(|f| f.id.clone()).unwrap_or_default()
                                });
                                Ok(MemoryOp::Update { id: candidate.id.clone(), old_id })
                            }
                            "delete" => {
                                let target = decision.target_id.unwrap_or_else(|| {
                                    existing.first().map(|f| f.id.clone()).unwrap_or_default()
                                });
                                Ok(MemoryOp::Delete { id: target, superseded_by: candidate.id.clone() })
                            }
                            "noop" | _ => Ok(MemoryOp::Noop),
                        }
                    }
                    Err(e) => {
                        tracing::warn!("LLM consolidation JSON parse failed: {e}, falling back");
                        self.fallback.decide(candidate, existing).await
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM consolidation call failed: {e}, falling back");
                self.fallback.decide(candidate, existing).await
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(llm_consolidation)'`
Expected: PASS.

---

### Task 6: LlmReflectionHandler

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_llm_reflection_parses_output() {
    let json_response = r#"{
        "fact_updates": [
            {"domain":"energy","subject":"user","predicate":"afternoon_dip","object":"energy drops after 3pm","confidence":0.8,"source":"reflected"}
        ],
        "rule_updates": [
            {"domain":"productivity","rule_text":"Suggest break at 3pm when energy declining","confidence":0.75}
        ],
        "summary":"User shows consistent afternoon energy decline. Exercise-productivity correlation observed."
    }"#;
    let mock = Arc::new(MockProvider::new(LlmResponse {
        content: Some(json_response.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
        usage: Default::default(),
        reasoning_content: None,
    }));
    let params = ChatParams::new("test-model").with_max_tokens(2048);
    let handler = LlmReflectionHandler::new(mock, params);

    let input = ReflectionInput {
        episodic_memories: vec![],
        user_model: UserModel::default(),
        procedural_rules: vec![],
        period_start: "2026-03-01".into(),
        period_end: "2026-03-07".into(),
    };
    let output = handler.reflect(&input).await.unwrap();
    assert_eq!(output.fact_updates.len(), 1);
    assert_eq!(output.fact_updates[0].predicate, "afternoon_dip");
    assert_eq!(output.rule_updates.len(), 1);
    assert!(output.summary.contains("afternoon energy decline"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(llm_reflection)'`
Expected: FAIL.

**Step 3: Implement**

```rust
fn reflection_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "fact_updates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "domain": { "type": "string" },
                        "subject": { "type": "string" },
                        "predicate": { "type": "string" },
                        "object": { "type": "string" },
                        "confidence": { "type": "number" },
                        "source": { "type": "string" }
                    },
                    "required": ["domain", "subject", "predicate", "object", "confidence", "source"],
                    "additionalProperties": false
                }
            },
            "rule_updates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "domain": { "type": "string" },
                        "rule_text": { "type": "string" },
                        "confidence": { "type": "number" }
                    },
                    "required": ["domain", "rule_text", "confidence"],
                    "additionalProperties": false
                }
            },
            "summary": { "type": "string" }
        },
        "required": ["fact_updates", "rule_updates", "summary"],
        "additionalProperties": false
    })
}

const REFLECTION_SYSTEM_PROMPT: &str = r#"You are a cognitive reflection agent performing weekly self-review. Analyze the user's episodic memories, current model, and procedural rules to identify:

1. Cross-domain patterns (e.g., exercise correlates with productivity)
2. Facts that should be updated based on new evidence
3. New procedural rules based on validated patterns (minimum 5 signals across 3+ days)
4. Facts that may be outdated and should be revisited

Output:
- fact_updates: New or updated semantic facts. Use source "reflected".
- rule_updates: New or updated procedural rules. Use domains: productivity, tasks, finance, coaching, general.
- summary: 2-3 sentence synthesis of the week's patterns.

Be conservative — only propose changes with strong evidence. Prefer updating existing facts over creating new ones."#;

/// LLM-backed weekly reflection.
pub struct LlmReflectionHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmReflectionHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonSchema {
                name: "ReflectionResult".into(),
                schema: reflection_schema(),
            }),
        }
    }
}

#[derive(serde::Deserialize)]
struct ReflectionResultJson {
    fact_updates: Vec<ReflectionFactJson>,
    rule_updates: Vec<ReflectionRuleJson>,
    summary: String,
}

#[derive(serde::Deserialize)]
struct ReflectionFactJson {
    domain: String,
    subject: String,
    predicate: String,
    object: String,
    confidence: f64,
    source: String,
}

#[derive(serde::Deserialize)]
struct ReflectionRuleJson {
    domain: String,
    rule_text: String,
    confidence: f64,
}

#[async_trait]
impl ReflectionHandler for LlmReflectionHandler {
    async fn reflect(&self, input: &ReflectionInput) -> common::Result<ReflectionOutput> {
        let memories_text: Vec<String> = input
            .episodic_memories
            .iter()
            .map(|m| format!("[{}] {}: {}", m.occurred_at, m.domain, m.content))
            .collect();

        let rules_text: Vec<String> = input
            .procedural_rules
            .iter()
            .map(|r| format!("[{}] {} (confidence: {:.0}%)", r.domain, r.rule_text, r.confidence * 100.0))
            .collect();

        let model_text = serde_json::to_string_pretty(&input.user_model).unwrap_or_default();

        let user_msg = format!(
            "Period: {} to {}\n\n## Episodic Memories ({})\n{}\n\n## Current User Model\n{}\n\n## Active Procedural Rules ({})\n{}",
            input.period_start, input.period_end,
            memories_text.len(), memories_text.join("\n"),
            model_text,
            rules_text.len(), rules_text.join("\n"),
        );

        let messages = vec![
            Message::system(REFLECTION_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        let response = self.provider.chat(&messages, None, &self.params).await?;
        let content = response.content.unwrap_or_default();
        let result: ReflectionResultJson = serde_json::from_str(&content)
            .map_err(|e| common::KlyntbotError::Generic(format!("Reflection JSON parse error: {e}")))?;

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let fact_updates = result
            .fact_updates
            .into_iter()
            .map(|f| SemanticFact {
                id: uuid::Uuid::new_v4().to_string(),
                domain: f.domain,
                subject: f.subject,
                predicate: f.predicate,
                object: f.object,
                confidence: f.confidence,
                source: f.source,
                valid_from: now.clone(),
                valid_until: None,
                recorded_at: now.clone(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
            })
            .collect();

        let rule_updates = result
            .rule_updates
            .into_iter()
            .map(|r| ProceduralRule {
                id: uuid::Uuid::new_v4().to_string(),
                domain: r.domain,
                rule_text: r.rule_text,
                confidence: r.confidence,
                source: "reflected".into(),
                signal_count: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                active: true,
            })
            .collect();

        Ok(ReflectionOutput {
            fact_updates,
            rule_updates,
            summary: result.summary,
        })
    }
}
```

**Step 4: Run test**

Run: `cargo nextest run -p agent -E 'test(llm_reflection)'`
Expected: PASS.

---

### Task 7: LlmCoachingReasonerHandler

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs`

**Step 1: Write the test**

```rust
#[tokio::test]
async fn test_llm_coaching_reasoner_parses_intervention() {
    use feature_coaching::reasoner::{CoachingReasonerHandler, ReasonerInput, InterventionType};
    use feature_coaching::signal_accumulator::TriggerFired;

    let mock = Arc::new(MockProvider::new(LlmResponse {
        content: Some(r#"{"should_intervene":true,"confidence":0.75,"message":"You've been distracted 3 times. A short walk might help.","intervention_type":"chat_message","reasoning":"Distraction pattern detected","observations":["Afternoon focus decline"]}"#.into()),
        tool_calls: vec![],
        finish_reason: "stop".into(),
        usage: Default::default(),
        reasoning_content: None,
    }));
    let params = ChatParams::new("test-model");
    let handler = LlmCoachingReasonerHandler::new(mock, params);

    let input = ReasonerInput {
        situation: UserSituation::default(),
        trigger: TriggerFired {
            condition_name: "distraction_streak".into(),
            confidence: 0.8,
            context: "3 distractions in 15min".into(),
        },
        patterns: vec![],
        relevant_memories: vec![],
        recent_interventions: vec![],
    };

    let decision = handler.reason(&input).await.unwrap();
    assert!(decision.should_intervene);
    assert!(decision.message.is_some());
    assert!((decision.confidence - 0.75).abs() < 0.01);
}

#[tokio::test]
async fn test_llm_coaching_reasoner_falls_back() {
    let mock = Arc::new(MockProvider::new_error("LLM down"));
    let params = ChatParams::new("test-model");
    let handler = LlmCoachingReasonerHandler::new(mock, params);

    let input = ReasonerInput {
        situation: UserSituation { coaching_receptivity: 0.7, ..Default::default() },
        trigger: TriggerFired {
            condition_name: "distraction_streak".into(),
            confidence: 0.8,
            context: "test".into(),
        },
        patterns: vec![],
        relevant_memories: vec![],
        recent_interventions: vec![],
    };

    let decision = handler.reason(&input).await.unwrap();
    // Should get heuristic result, not an error
    assert!(decision.should_intervene);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(llm_coaching)'`
Expected: FAIL.

**Step 3: Implement**

```rust
use feature_coaching::reasoner::{
    CoachingDecision, CoachingReasonerHandler, InterventionType, ReasonerInput,
};

fn coaching_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "should_intervene": { "type": "boolean" },
            "confidence": { "type": "number" },
            "message": { "type": ["string", "null"] },
            "intervention_type": { "type": "string", "enum": ["dashboard_card", "chat_message", "notification", "overlay", "none"] },
            "reasoning": { "type": "string" },
            "observations": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["should_intervene", "confidence", "intervention_type", "reasoning", "observations"],
        "additionalProperties": false
    })
}

const COACHING_SYSTEM_PROMPT: &str = r#"You are a proactive coaching agent. Given the user's current situation, a triggered condition, detected patterns, and relevant memories, decide whether and how to intervene.

Principles:
- Be helpful, not annoying. Respect the user's flow.
- Don't interrupt deep focus for low-priority nudges.
- Consider coaching_receptivity: below 0.3 means the user doesn't engage with nudges.
- Personalize the message based on patterns and memories.
- Keep messages concise (1-2 sentences).

Intervention types (from least to most intrusive):
- dashboard_card: Subtle, shown on dashboard
- chat_message: Sent as a chat message
- notification: System notification
- overlay: Full-screen overlay (only for critical situations)
- none: No intervention

Set should_intervene to false if unsure or if the user would likely dismiss it."#;

/// LLM-backed coaching reasoner with heuristic fallback.
pub struct LlmCoachingReasonerHandler {
    provider: DynProvider,
    params: ChatParams,
}

impl LlmCoachingReasonerHandler {
    pub fn new(provider: DynProvider, params: ChatParams) -> Self {
        Self {
            provider,
            params: params.with_response_format(ResponseFormat::JsonSchema {
                name: "CoachingDecision".into(),
                schema: coaching_schema(),
            }),
        }
    }
}

#[derive(serde::Deserialize)]
struct CoachingDecisionJson {
    should_intervene: bool,
    confidence: f64,
    message: Option<String>,
    intervention_type: String,
    reasoning: String,
    observations: Vec<String>,
}

#[async_trait]
impl CoachingReasonerHandler for LlmCoachingReasonerHandler {
    async fn reason(&self, input: &ReasonerInput) -> common::Result<CoachingDecision> {
        let patterns_text: Vec<String> = input
            .patterns
            .iter()
            .map(|p| format!("{}: {} (confidence: {:.0}%)", p.name, p.description, p.confidence * 100.0))
            .collect();

        let user_msg = format!(
            "## Current Situation\n\
             Energy: {:.0}%, Focus: {:.0}%, Deadline pressure: {:.0}%\n\
             Distraction risk: {:.0}%, Coaching receptivity: {:.0}%\n\
             Hours active: {:.1}h, Since break: {:.0}min, Context switches: {}\n\
             Task avoidance: {}\n\n\
             ## Trigger\n{}: {} (confidence: {:.0}%)\n\n\
             ## Detected Patterns ({})\n{}\n\n\
             ## Relevant Memories\n{}\n\n\
             ## Recent Interventions\n{}",
            input.situation.energy_level * 100.0,
            input.situation.focus_state * 100.0,
            input.situation.deadline_pressure * 100.0,
            input.situation.distraction_risk * 100.0,
            input.situation.coaching_receptivity * 100.0,
            input.situation.hours_active_today,
            input.situation.mins_since_break,
            input.situation.recent_context_switches,
            input.situation.task_avoidance_detected,
            input.trigger.condition_name, input.trigger.context, input.trigger.confidence * 100.0,
            patterns_text.len(), patterns_text.join("\n"),
            if input.relevant_memories.is_empty() { "None".into() } else { input.relevant_memories.join("\n") },
            if input.recent_interventions.is_empty() { "None".into() } else { input.recent_interventions.join("\n") },
        );

        let messages = vec![
            Message::system(COACHING_SYSTEM_PROMPT),
            Message::user(user_msg),
        ];

        match self.provider.chat(&messages, None, &self.params).await {
            Ok(response) => {
                let content = response.content.unwrap_or_default();
                match serde_json::from_str::<CoachingDecisionJson>(&content) {
                    Ok(d) => Ok(CoachingDecision {
                        should_intervene: d.should_intervene,
                        confidence: d.confidence,
                        message: d.message,
                        intervention_type: match d.intervention_type.as_str() {
                            "dashboard_card" => InterventionType::DashboardCard,
                            "chat_message" => InterventionType::ChatMessage,
                            "notification" => InterventionType::Notification,
                            "overlay" => InterventionType::Overlay,
                            _ => InterventionType::None,
                        },
                        reasoning: d.reasoning,
                        observations: d.observations,
                    }),
                    Err(e) => {
                        tracing::warn!("LLM coaching JSON parse failed: {e}, falling back");
                        Ok(feature_coaching::reasoner::heuristic_reason(input))
                    }
                }
            }
            Err(e) => {
                tracing::warn!("LLM coaching call failed: {e}, falling back");
                Ok(feature_coaching::reasoner::heuristic_reason(input))
            }
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(llm_coaching)'`
Expected: PASS.

---

### Task 8: CoachingService

**Files:**
- Create: `crates/feature-coaching/src/service.rs`
- Modify: `crates/feature-coaching/src/lib.rs` (add module + exports)

**Step 1: Write the test**

Add test at bottom of `service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoner::{CoachingDecision, InterventionType};

    struct MockReasoner {
        decision: CoachingDecision,
    }

    #[async_trait::async_trait]
    impl CoachingReasonerHandler for MockReasoner {
        async fn reason(&self, _input: &ReasonerInput) -> common::Result<CoachingDecision> {
            Ok(self.decision.clone())
        }
    }

    #[tokio::test]
    async fn test_coaching_service_processes_distraction_events() {
        let accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let detector = Arc::new(Mutex::new(PatternDetector::new()));
        let router = Arc::new(Mutex::new(InterventionRouter::default()));
        let feedback = Arc::new(Mutex::new(FeedbackTracker::new()));
        let situation = Arc::new(Mutex::new(UserSituation {
            coaching_receptivity: 0.7,
            ..Default::default()
        }));

        let reasoner: Arc<dyn CoachingReasonerHandler> = Arc::new(MockReasoner {
            decision: CoachingDecision {
                should_intervene: true,
                confidence: 0.8,
                message: Some("Take a break!".into()),
                intervention_type: InterventionType::ChatMessage,
                reasoning: "test".into(),
                observations: vec![],
            },
        });

        let (intervention_tx, mut intervention_rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();
        let bus = bus::DomainEventBus::new(16);
        let event_rx = bus.subscribe();

        let _service = CoachingService::start(
            event_rx, accumulator, detector, router, feedback,
            situation, reasoner, intervention_tx, cancel.clone(),
        );

        // Push 3 distraction events to trigger distraction_streak
        for _ in 0..3 {
            bus.publish(bus::DomainEvent::DistractionDetected {
                app: "reddit".into(),
                duration_secs: None,
                context: "test".into(),
            });
        }

        // Wait briefly for processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Should have received an intervention
        let intervention = intervention_rx.try_recv();
        assert!(intervention.is_ok(), "Expected an intervention to be delivered");
        assert_eq!(intervention.unwrap().message, "Take a break!");

        cancel.cancel();
    }

    #[tokio::test]
    async fn test_coaching_service_stops_gracefully() {
        let accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let detector = Arc::new(Mutex::new(PatternDetector::new()));
        let router = Arc::new(Mutex::new(InterventionRouter::default()));
        let feedback = Arc::new(Mutex::new(FeedbackTracker::new()));
        let situation = Arc::new(Mutex::new(UserSituation::default()));
        let reasoner: Arc<dyn CoachingReasonerHandler> = Arc::new(MockReasoner {
            decision: CoachingDecision {
                should_intervene: false,
                confidence: 0.0,
                message: None,
                intervention_type: InterventionType::None,
                reasoning: "test".into(),
                observations: vec![],
            },
        });

        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();
        let bus = bus::DomainEventBus::new(16);

        let mut service = CoachingService::start(
            bus.subscribe(), accumulator, detector, router, feedback,
            situation, reasoner, tx, cancel.clone(),
        );

        service.stop().await;
        // Should not panic or hang
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p feature-coaching -E 'test(coaching_service)'`
Expected: FAIL — module doesn't exist.

**Step 3: Implement**

Create `crates/feature-coaching/src/service.rs`:

```rust
//! CoachingService — subscribes to DomainEventBus and runs the full coaching
//! pipeline: signal accumulation → trigger evaluation → pattern detection →
//! reasoning → intervention routing → feedback tracking.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use bus::DomainEvent;
use cognitive::situation::UserSituation;

use crate::feedback::FeedbackTracker;
use crate::pattern_detector::PatternDetector;
use crate::reasoner::{CoachingReasonerHandler, ReasonerInput};
use crate::router::{DeliveredIntervention, InterventionRouter, RoutingResult};
use crate::signal_accumulator::SignalAccumulator;

/// Background service that processes domain events through the coaching pipeline.
pub struct CoachingService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl CoachingService {
    pub fn start(
        mut event_rx: broadcast::Receiver<DomainEvent>,
        accumulator: Arc<Mutex<SignalAccumulator>>,
        detector: Arc<Mutex<PatternDetector>>,
        router: Arc<Mutex<InterventionRouter>>,
        feedback: Arc<Mutex<FeedbackTracker>>,
        situation: Arc<Mutex<UserSituation>>,
        reasoner: Arc<dyn CoachingReasonerHandler>,
        intervention_tx: mpsc::Sender<DeliveredIntervention>,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = event_rx.recv() => {
                        let event = match result {
                            Ok(e) => e,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("CoachingService lagged, skipped {n} events");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        // 1. Push signal into accumulator
                        {
                            let mut acc = accumulator.lock().await;
                            acc.push_event(&event);
                        }

                        // 2. Incrementally update situation from event
                        update_situation_from_event(&situation, &event).await;

                        // 3. Evaluate triggers
                        let sit = situation.lock().await.clone();
                        let fired = {
                            let mut acc = accumulator.lock().await;
                            acc.evaluate(&sit)
                        };

                        if fired.is_empty() {
                            continue;
                        }

                        // 4. Process each fired trigger
                        for trigger in fired {
                            // Record in pattern detector
                            {
                                let mut det = detector.lock().await;
                                det.record_trigger(&trigger);
                            }

                            // Detect patterns
                            let patterns = {
                                let det = detector.lock().await;
                                det.detect_patterns()
                            };

                            // Build reasoner input
                            let input = ReasonerInput {
                                situation: sit.clone(),
                                trigger: trigger.clone(),
                                patterns,
                                relevant_memories: vec![], // Future: retrieve from cognitive memory
                                recent_interventions: vec![],
                            };

                            // Call reasoner
                            let decision = match reasoner.reason(&input).await {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("Coaching reasoner failed: {e}");
                                    continue;
                                }
                            };

                            // Route intervention
                            let routing = {
                                let mut r = router.lock().await;
                                r.route(&decision, &trigger.condition_name)
                            };

                            match routing {
                                RoutingResult::Delivered(intervention) => {
                                    debug!(
                                        "Coaching intervention delivered: {} via {:?}",
                                        trigger.condition_name, intervention.intervention_type
                                    );
                                    // Record in feedback tracker
                                    {
                                        let mut fb = feedback.lock().await;
                                        fb.record_delivery(&intervention);
                                    }
                                    // Send to consumer
                                    let _ = intervention_tx.send(intervention).await;
                                }
                                RoutingResult::RateLimited { reason } => {
                                    debug!("Coaching intervention rate-limited: {reason}");
                                }
                                RoutingResult::Skipped => {}
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("CoachingService task panicked: {e}");
            }
        }
    }
}

/// Incrementally update UserSituation from a domain event.
async fn update_situation_from_event(situation: &Arc<Mutex<UserSituation>>, event: &DomainEvent) {
    let mut sit = situation.lock().await;
    match event {
        DomainEvent::DistractionDetected { .. } => {
            sit.distraction_risk = (sit.distraction_risk + 0.15).min(1.0);
        }
        DomainEvent::FocusSessionEnded { quality, .. } => {
            sit.focus_state = *quality;
        }
        DomainEvent::TaskDeferred { .. } => {
            sit.task_avoidance_detected = true;
        }
        DomainEvent::BudgetAlert { .. } => {
            sit.deadline_pressure = (sit.deadline_pressure + 0.2).min(1.0);
        }
        DomainEvent::ActivitySessionCompleted { .. } => {
            sit.hours_active_today += 0.5; // Approximate
        }
        _ => {}
    }
}
```

**Step 4: Update lib.rs exports**

In `crates/feature-coaching/src/lib.rs`, add:

```rust
pub mod service;
pub use service::CoachingService;
```

**Step 5: Run tests**

Run: `cargo nextest run -p feature-coaching -E 'test(coaching_service)'`
Expected: PASS.

---

### Task 9: Wire handlers in AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Add with_cognitive_provider method**

Add a new field to `AgentLoopBuilder` struct (after `domain_event_bus` field at line 59):

```rust
    cognitive_provider: Option<providers::DynProvider>,
```

Add builder method (after `with_domain_bus` at line 99):

```rust
    pub fn with_cognitive_provider(mut self, provider: Option<providers::DynProvider>) -> Self {
        self.cognitive_provider = provider;
        self
    }
```

Initialize in `new()` (around line 64):

```rust
    cognitive_provider: None,
```

**Step 2: Swap handler selection in build()**

Replace the handler creation block (lines 235-238) from:

```rust
let extraction: Arc<dyn cognitive::ExtractionHandler> =
    Arc::new(crate::cognitive_handlers::HeuristicExtractionHandler);
let consolidation: Arc<dyn cognitive::ConsolidationHandler> =
    Arc::new(crate::cognitive_handlers::HeuristicConsolidationHandler);
```

To:

```rust
let (extraction, consolidation): (
    Arc<dyn cognitive::ExtractionHandler>,
    Arc<dyn cognitive::ConsolidationHandler>,
) = if let Some(ref cp) = self.cognitive_provider {
    let params = providers::cognitive_chat_params(&self.config, 1024);
    (
        Arc::new(crate::cognitive_handlers::LlmExtractionHandler::new(
            cp.clone(), params.clone(),
        )),
        Arc::new(crate::cognitive_handlers::LlmConsolidationHandler::new(
            cp.clone(), params,
        )),
    )
} else {
    (
        Arc::new(crate::cognitive_handlers::HeuristicExtractionHandler),
        Arc::new(crate::cognitive_handlers::HeuristicConsolidationHandler),
    )
};
```

**Step 3: Verify**

Run: `cargo build -p agent`
Expected: compiles.

Run: `cargo nextest run -p agent`
Expected: all existing tests pass (they don't set cognitive_provider, so heuristic path is used).

---

### Task 10: Wire CoachingService and reflection in AppCore

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

This is the most complex wiring task. Changes to `AppCore::init()`:

**Step 1: Add coaching_service field to AppCore struct**

After the `user_situation` field (around line 72), add:

```rust
    coaching_service: Option<CoachingService>,
```

**Step 2: Create cognitive provider in init()**

After the agent builder section (around line 159, after `let agent = builder.build().await?;`), but before the coaching component initialization (line 349):

```rust
// Create cognitive provider for LLM-backed handlers
let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
if cognitive_provider.is_some() {
    info!("Cognitive provider created — using LLM handlers");
} else {
    info!("No cognitive provider — using heuristic handlers");
}
```

Also pass it to the builder (modify line 155-159):

```rust
let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();

let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
    .with_pool(storage_pool.inner().clone())
    .with_cron_service(cron_service.clone())
    .with_notification_handle(notification_dispatcher.last_active_handle())
    .with_domain_bus(Arc::clone(&domain_event_bus))
    .with_cognitive_provider(cognitive_provider.clone());
```

**Step 3: Start CoachingService after coaching components are created**

After line 353 (`let user_situation = ...`), add:

```rust
// Select coaching reasoner (LLM or heuristic)
let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
    if let Some(ref cp) = cognitive_provider {
        let params = providers::cognitive_chat_params(&config, 1024);
        Arc::new(agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
            cp.clone(), params,
        ))
    } else {
        // Wrap heuristic_reason in a trivial handler struct
        Arc::new(agent::cognitive_handlers::HeuristicCoachingReasonerHandler)
    };

// Start CoachingService
let (intervention_tx, mut intervention_rx) = tokio::sync::mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);
let coaching_cancel = tokio_util::sync::CancellationToken::new();
let coaching_service = feature_coaching::CoachingService::start(
    domain_event_bus.subscribe(),
    signal_accumulator.clone(),
    pattern_detector.clone(),
    intervention_router.clone(),
    feedback_tracker.clone(),
    user_situation.clone(),
    coaching_reasoner,
    intervention_tx,
    coaching_cancel.clone(),
);
info!("Coaching service started");

// Forward interventions to Tauri frontend
{
    let app_handle_for_coaching = app_handle.clone();
    tokio::spawn(async move {
        while let Some(intervention) = intervention_rx.recv().await {
            let _ = app_handle_for_coaching.emit("coaching:intervention", &intervention);
        }
    });
}
```

Note: This also requires a `HeuristicCoachingReasonerHandler` wrapper struct in `cognitive_handlers.rs` that implements `CoachingReasonerHandler` by calling `heuristic_reason()`. Add this in `cognitive_handlers.rs`:

```rust
/// Wrapper to use heuristic_reason as a CoachingReasonerHandler impl.
pub struct HeuristicCoachingReasonerHandler;

#[async_trait]
impl CoachingReasonerHandler for HeuristicCoachingReasonerHandler {
    async fn reason(&self, input: &ReasonerInput) -> common::Result<CoachingDecision> {
        Ok(feature_coaching::reasoner::heuristic_reason(input))
    }
}
```

**Step 4: Add coaching_service to Self construction**

In the `let core = Self { ... }` block (around line 407-431), add:

```rust
    coaching_service: Some(coaching_service),
```

**Step 5: Schedule weekly reflection**

After the coaching service start, add reflection scheduling:

```rust
// Schedule weekly reflection via CronService
{
    let reflection_schedule = config
        .cognitive
        .reflection_schedule
        .as_deref()
        .unwrap_or("0 9 * * 1"); // Monday 9am default

    let fact_repo = cognitive::SemanticFactRepo::new(storage_pool.inner().clone());
    let episodic_repo = cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone());

    let reflection_handler: Arc<dyn cognitive::ReflectionHandler> =
        if let Some(ref cp) = cognitive_provider {
            let params = providers::cognitive_chat_params(&config, 2048);
            Arc::new(agent::cognitive_handlers::LlmReflectionHandler::new(cp.clone(), params))
        } else {
            Arc::new(agent::cognitive_handlers::HeuristicReflectionHandler)
        };

    let consolidation_handler: Arc<dyn cognitive::ConsolidationHandler> =
        if let Some(ref cp) = cognitive_provider {
            let params = providers::cognitive_chat_params(&config, 1024);
            Arc::new(agent::cognitive_handlers::LlmConsolidationHandler::new(cp.clone(), params))
        } else {
            Arc::new(agent::cognitive_handlers::HeuristicConsolidationHandler)
        };

    // Register the reflection job
    if let Err(e) = cron_service
        .add_job(
            "cognitive_weekly_reflection",
            scheduling::CronSchedule::Cron {
                expr: reflection_schedule.to_string(),
                tz: Some(config.timezone.clone()),
            },
            "Weekly cognitive reflection",
            false,
            None,
            None,
            false,
        )
        .await
    {
        warn!("Failed to schedule weekly reflection: {e}");
    }
}
```

Note: The actual reflection execution happens in the cron callback. This requires adding a handler in the cron callback setup (in `crates/cli/src/serve.rs` or wherever the callback is set). The cron job name `"cognitive_weekly_reflection"` should be matched in the callback to call `run_weekly_reflection()`.

**Step 6: Verify**

Run: `cargo build -p desktop`
Expected: compiles (there will be unused import warnings to clean up).

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 new warnings.

---

### Task 11: Dev-API cognitive provider wiring

**Files:**
- Modify: `crates/dev-api/src/main.rs`

**Step 1: Create cognitive provider in dev-api startup**

In the main function, after the agent builder is created (around line 136), add cognitive provider:

```rust
let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();

let mut builder = agent::AgentLoop::builder(bus.clone(), provider, config.clone())
    .with_pool(pool.inner().clone())
    .with_cron_service(cron_service.clone())
    .with_domain_bus(Arc::clone(&domain_event_bus))
    .with_cognitive_provider(cognitive_provider.clone());
```

The dev-api already has all the cognitive endpoints — this just ensures the `BackgroundConsolidationService` inside the agent loop uses LLM handlers when a cognitive provider is configured.

**Step 2: Verify**

Run: `cargo build -p dev-api`
Expected: compiles.

---

### Task 12: MockProvider for tests + full verification

**Files:**
- Modify: `crates/agent/src/cognitive_handlers.rs` (test utilities)

**Step 1: Add MockProvider to cognitive_handlers tests**

The test module in `cognitive_handlers.rs` needs a `MockProvider` that implements `LlmProvider`. Check if there's an existing one to reuse from `crates/agent/src/agent_loop/refactor_tests.rs`. If it's not public, create a minimal one in the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use providers::{LlmResponse, ProviderCapabilities, ProviderHealth};

    struct MockProvider {
        response: Result<LlmResponse, String>,
    }

    impl MockProvider {
        fn new(response: LlmResponse) -> Self {
            Self { response: Ok(response) }
        }

        fn new_error(msg: &str) -> Self {
            Self { response: Err(msg.into()) }
        }
    }

    #[async_trait]
    impl providers::LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            match &self.response {
                Ok(r) => Ok(r.clone()),
                Err(e) => Err(common::KlyntbotError::Generic(e.clone())),
            }
        }

        async fn chat_stream(
            &self,
            _messages: &[Message],
            _tools: Option<&[serde_json::Value]>,
            _params: &ChatParams,
        ) -> common::Result<providers::LlmStream> {
            unimplemented!("mock doesn't support streaming")
        }

        fn supports_streaming(&self) -> bool { false }
        fn default_model(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
        async fn count_tokens(&self, _: &[Message], _: Option<&[serde_json::Value]>) -> common::Result<usize> { Ok(0) }
        fn capabilities(&self) -> ProviderCapabilities { ProviderCapabilities::default() }
        fn context_window(&self) -> usize { 128000 }
        async fn health_check(&self) -> common::Result<ProviderHealth> {
            Ok(ProviderHealth { healthy: true, latency_ms: 0, message: None })
        }
    }

    // ... all the test functions from Tasks 3-7 go here ...
}
```

**Step 2: Full verification**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 new warnings.

Run: `cargo nextest run --workspace`
Expected: all tests pass.

Run: `cargo fmt --all --check`
Expected: no formatting issues.

---

## Execution Order Summary

| Task | What | Depends On |
|------|------|-----------|
| 1 | CognitiveConfig | — |
| 2 | create_cognitive_provider() | Task 1 |
| 3 | HeuristicReflectionHandler | — |
| 4 | LlmExtractionHandler | Task 2 |
| 5 | LlmConsolidationHandler | Task 2 |
| 6 | LlmReflectionHandler | Task 2 |
| 7 | LlmCoachingReasonerHandler | Task 2 |
| 8 | CoachingService | — |
| 9 | Wire in AgentLoopBuilder | Tasks 2, 4, 5 |
| 10 | Wire in AppCore | Tasks 3-9 |
| 11 | Dev-API wiring | Task 2 |
| 12 | MockProvider + full verification | Tasks 3-11 |

Tasks 1-2 must go first (config + provider). Tasks 3-8 can be done in parallel. Tasks 9-11 are wiring that depends on the handlers. Task 12 is final verification.
