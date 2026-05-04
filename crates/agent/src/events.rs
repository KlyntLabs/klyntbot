//! Agent event types for real-time streaming updates.
//!
//! These events are emitted by the agent loop during processing,
//! allowing consumers (like the CLI) to display real-time progress.

use serde::{Deserialize, Serialize};

/// Events emitted by the agent loop during processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum AgentEvent {
    /// A chunk of content streamed from the LLM.
    ContentChunk { data: String },

    /// Pipeline processing has begun (emitted immediately, before classification).
    PipelineStarted,

    /// A tool execution has started.
    ToolStart {
        name: String,
        args: serde_json::Value,
        /// Which agent initiated this tool (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A tool execution has completed.
    ToolEnd {
        name: String,
        success: bool,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
        /// Truncated result of the tool execution (max 2KB).
        result: Option<String>,
        /// Which agent initiated this tool (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A new agent iteration has started.
    IterationStart { iteration: usize, max: usize },

    /// Context assembly step completed.
    ContextAssembled {
        #[serde(rename = "totalTokens")]
        total_tokens: usize,
        budget: usize,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// Tiered history compression completed during context assembly.
    ContextTieredCompressed {
        /// Number of turns kept verbatim (Tier 0).
        #[serde(rename = "tier0Kept")]
        tier0_kept: usize,
        /// Total tokens in Tier 1 (detailed) summaries.
        #[serde(rename = "tier1Tokens")]
        tier1_tokens: usize,
        /// Total tokens in Tier 2 (condensed) summaries.
        #[serde(rename = "tier2Tokens")]
        tier2_tokens: usize,
        /// Whether cognitive scoring was used for tier assignment.
        #[serde(rename = "cognitiveScoringUsed")]
        cognitive_scoring_used: bool,
        /// Whether delta-only compression was used.
        #[serde(rename = "deltaOnly")]
        delta_only: bool,
    },

    /// Query enhancement pipeline completed — emits the trace for observability.
    RetrievalEnhanced {
        stages: Vec<context_engine::enhancement::StageTrace>,
        #[serde(rename = "totalLatencyMs")]
        total_latency_ms: u64,
        #[serde(rename = "totalLlmCalls")]
        total_llm_calls: u32,
    },

    /// Execution engine selected and starting.
    ExecutionStarted {
        engine: String,
        #[serde(rename = "maxIterations")]
        max_iterations: usize,
    },

    /// Processing is complete with the final accumulated content.
    Done {
        content: String,
        /// The persisted assistant message ID (for targeted metadata updates).
        #[serde(rename = "messageId", skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
    },

    /// Internal confidence assessment completed (not shown to user in CLI).
    ConfidenceAssessed { score: f32, action: String },

    /// An error occurred during processing.
    Error { message: String },

    /// An entity was created by a tool (task, project, area, etc.).
    EntityCreated(common::EntityCard),

    /// Token usage report after cost tracking.
    UsageReport {
        #[serde(rename = "promptTokens")]
        prompt_tokens: u32,
        #[serde(rename = "completionTokens")]
        completion_tokens: u32,
        #[serde(rename = "cacheReadTokens")]
        cache_read_tokens: u32,
        #[serde(rename = "cacheWriteTokens")]
        cache_write_tokens: u32,
        #[serde(rename = "estimatedCostUsd")]
        estimated_cost_usd: f64,
        model: String,
        #[serde(rename = "responseTimeMs")]
        response_time_ms: u64,
    },

    /// Monthly LLM cost budget warning (emitted at 80% and 100%).
    BudgetWarning {
        #[serde(rename = "monthlySpendUsd")]
        monthly_spend_usd: f64,
        #[serde(rename = "monthlyBudgetUsd")]
        monthly_budget_usd: f64,
        #[serde(rename = "usagePercent")]
        usage_percent: f64,
    },

    /// A memory search or operation was performed.
    MemoryAccess {
        action: String,
        query: Option<String>,
        #[serde(rename = "resultsCount")]
        results_count: u32,
    },

    /// A skill was loaded into the system prompt.
    SkillLoaded {
        name: String,
        trigger: String,
        /// Which agent this skill belongs to (set during delegation).
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },

    /// A learning event occurred (threshold adjustment, pattern detection).
    LearningEvent {
        #[serde(rename = "eventType")]
        event_type: String,
        detail: String,
    },

    /// An agent profile was selected to handle the current message.
    AgentSelected { name: String, description: String },

    /// A subagent was spawned.
    SubagentSpawned {
        #[serde(rename = "agentId")]
        agent_id: String,
        label: String,
        profile: String,
        #[serde(rename = "parentSessionId")]
        parent_session_id: String,
        #[serde(rename = "spawnedAt")]
        spawned_at: i64,
    },

    /// A subagent reported per-iteration progress.
    SubagentProgress {
        #[serde(rename = "agentId")]
        agent_id: String,
        iteration: u32,
        #[serde(rename = "lastTool")]
        last_tool: Option<String>,
    },

    /// A subagent completed (success or error).
    SubagentCompleted {
        #[serde(rename = "agentId")]
        agent_id: String,
        success: bool,
        summary: String,
        #[serde(rename = "tokensUsed")]
        tokens_used: u64,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// A subagent was cancelled.
    SubagentCancelled {
        #[serde(rename = "agentId")]
        agent_id: String,
        reason: String,
        #[serde(rename = "cancelledAt")]
        cancelled_at: i64,
    },

    /// An agent delegation has started (agent-to-agent handoff).
    DelegationStarted {
        #[serde(rename = "fromAgent")]
        from_agent: String,
        #[serde(rename = "toAgent")]
        to_agent: String,
        query: String,
        depth: u32,
    },

    /// An agent delegation has completed.
    DelegationCompleted {
        #[serde(rename = "fromAgent")]
        from_agent: String,
        #[serde(rename = "toAgent")]
        to_agent: String,
        success: bool,
        #[serde(rename = "durationMs")]
        duration_ms: u64,
    },

    /// An MCP server connection status changed during startup.
    McpServerStatus {
        #[serde(rename = "serverName")]
        server_name: String,
        /// One of: "starting", "ready", "failed", "skipped"
        status: String,
        /// Number of tools discovered (only for "ready" status).
        #[serde(rename = "toolCount", skip_serializing_if = "Option::is_none")]
        tool_count: Option<usize>,
        /// Error message (only for "failed" status).
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },

    /// All MCP server connections have completed.
    McpStartupComplete {
        ready: usize,
        failed: usize,
        skipped: usize,
    },

    /// Chain-of-thought planning has started for a complex task.
    PlanningStarted {
        #[serde(rename = "complexityScore")]
        complexity_score: u8,
    },

    /// A structured execution plan was generated.
    PlanGenerated {
        steps: Vec<String>,
        #[serde(rename = "rawPlan")]
        raw_plan: String,
    },

    /// A plan step was completed during execution.
    PlanStepCompleted {
        #[serde(rename = "stepIndex")]
        step_index: usize,
        description: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },

    /// Live context was injected mid-execution (e.g., memory promoted during ReAct loop).
    ContextReassembled {
        updates: Vec<crate::execution::live_context_refresher::ContextReassembledUpdate>,
        #[serde(rename = "tokensAdded")]
        tokens_added: usize,
    },

    /// Mid-loop context compression was triggered during a reactive execution.
    ContextCompressed {
        #[serde(rename = "beforeTokens")]
        before_tokens: usize,
        #[serde(rename = "afterTokens")]
        after_tokens: usize,
        iteration: usize,
    },

    /// A memory was promoted from one scope to a higher scope.
    MemoryPromoted {
        #[serde(rename = "factId")]
        fact_id: String,
        #[serde(rename = "fromScope")]
        from_scope: String,
        #[serde(rename = "toScope")]
        to_scope: String,
        subject: String,
        predicate: String,
    },

    /// AutoTuner nightly cycle report.
    AutoTunerReport(::autotuner::AutoTunerReport),
    /// AutoTuner promoted a new champion.
    AutoTunerPromotion(::autotuner::AutoTunerPromotion),
    /// AutoTuner auto-reverted after regression.
    AutoTunerRollback(::autotuner::AutoTunerRollback),

    /// The agent detected a repeating tool call pattern (warning level).
    LoopDetected {
        iteration: usize,
        #[serde(rename = "toolsSummary")]
        tools_summary: String,
        suggestion: String,
    },

    /// The agent hit the hard-stop threshold for repeated tool calls.
    LoopHardStop {
        iteration: usize,
        #[serde(rename = "toolsSummary")]
        tools_summary: String,
    },

    // ── Budget HUD (Deep/Ultra mode) ─────────────────────────
    /// Emitted after each turn with current budget state.
    /// UI renders the live budget HUD from this.
    BudgetUpdate {
        tokens_remaining_pct: f32,
        turns_used: u32,
        max_turns: u32,
        cost_usd: f64,
        depth: String,
    },

    /// User extended the budget mid-conversation.
    BudgetExtended {
        additional_turns: u32,
        new_max_turns: u32,
    },

    // ── Depth suggestion (adaptive layer) ────────────────────
    /// Mirror/history suggests a different depth mode.
    DepthSuggestion { recommended: String, reason: String },

    // ── Enrichment progress (Phase 4) ────────────────────────
    /// Post-response enrichment started (Mirror, Coaching, NoteTree, FSRS).
    EnrichmentStarted { phase: String },

    /// Post-response enrichment completed.
    EnrichmentComplete { phase: String, summary: String },

    // ── Turn tracking ────────────────────────────────────────
    /// Emitted at the end of each execute-loop turn.
    TurnComplete {
        turn: u32,
        budget_remaining_pct: f32,
    },

    /// Recall context injected into the system prompt.
    RecallInjected {
        memory_ids: Vec<String>,
        #[serde(rename = "coverageScore")]
        coverage_score: f64,
        escalation_chain: Vec<String>,
        dead_end_warning: bool,
        #[serde(rename = "budgetUsedTokens")]
        budget_used_tokens: u32,
        #[serde(rename = "budgetLimitTokens")]
        budget_limit_tokens: u32,
    },

    /// Mirror flagged the user's current approach as a dead-end pattern.
    DeadEndWarningSurfaced {
        approach_summary: String,
        prior_attempt_id: String,
        confidence: f64,
    },

    /// SkillRouter evaluated a skill for activation; emit regardless of outcome.
    SkillActivationConsidered {
        skill_id: String,
        score: f64,
        threshold: f64,
        accepted: bool,
        decision_reason: String,
    },

    /// A skill activated and its frontmatter is being injected.
    SkillActivated {
        #[serde(rename = "skillId")]
        skill_id: String,
        source_path: String,
        trigger: String,
        injected_tokens: u32,
    },

    /// Agent loaded a referenced skill page on-demand.
    SkillReferenceLoaded {
        skill_id: String,
        reference: String,
        tokens: u32,
        load_kind: String,
    },

    /// Context engine finalized which sources to include for this turn.
    ContextEngineDecision {
        included: Vec<String>,
        excluded: Vec<String>,
        total_tokens: u32,
        budget_used_pct: f64,
    },

    /// Approval gate evaluated. Fires for every gate evaluation, not only
    /// "ask" cases. `requires_user_input` distinguishes auto-allow / auto-deny
    /// (false) from cases that need a chat-inline ApprovalCard (true).
    ApprovalRequested {
        request_id: String,
        tool: String,
        args_hash: String,
        layer: String,
        rule_matched: Option<String>,
        mirror_history: Option<serde_json::Value>,
        sandbox_summary: String,
        #[serde(rename = "requiresUserInput")]
        requires_user_input: bool,
        args: Option<serde_json::Value>,
        cwd: Option<String>,
        layer_reason: Option<String>,
    },

    /// Approval gate resolved (paired with ApprovalRequested by request_id).
    ApprovalResolved {
        request_id: String,
        decision: String,
        decision_reason: String,
        latency_ms: u64,
        persisted_rule: Option<String>,
        #[serde(rename = "decidedBy")]
        /// One of: user | auto_allow | auto_deny | timeout | cancelled.
        decided_by: String,
    },

    /// Plan mode changed for a session.
    PlanModeChanged {
        session_key: String,
        active: bool,
        requested_by: String,
    },

    /// Sandbox policy applied for a tool invocation.
    SandboxPolicyApplied {
        tool: String,
        policy_summary: String,
        policy_hash: String,
        #[serde(rename = "fallbackUnsandboxed")]
        fallback_unsandboxed: bool,
        fs_constraints: Vec<String>,
        network_constraints: Vec<String>,
    },

    /// A chunk of a streaming tool result (e.g., bash stdout/stderr lines).
    ToolCallStreamChunk {
        tool: String,
        chunk_kind: String, // "stdout" | "stderr" | "json"
        bytes: u64,
        truncated: bool,
    },

    /// MCP gateway subcall trace for telemetry.
    MCPSubcallTrace {
        server: String,
        tool: String,
        latency_ms: u64,
        bytes_returned: u64,
        error: Option<String>,
    },

    /// Provider request issued (one per iteration; can repeat on retry).
    ProviderRequest {
        iteration: u32,
        model: String,
        prompt_tokens: u32,
        max_tokens: u32,
        attempt: u32,
    },

    /// Provider response received (paired with ProviderRequest by ordering).
    ProviderResponse {
        #[serde(rename = "latencyMs")]
        latency_ms: u64,
        usage: serde_json::Value,
        cost_usd: f64,
        retries_used: u32,
        finish_reason: String,
    },

    /// MidLoopCompressor compacted the message history.
    MidLoopCompressionTriggered {
        before_tokens: u32,
        after_tokens: u32,
        messages_condensed: u32,
        regions: Vec<String>,
    },

    /// File edit with optional symbol/LSP enrichment. Phase 1 emits
    /// empty anchored_symbols (best-effort tree-sitter pass) and empty
    /// lsp_diagnostics_delta (LSP integration is Phase 2+).
    FileEditWithSymbols {
        path: String,
        op: String, // "edit" | "write" | "apply_patch"
        bytes: u64,
        diff_full: String,
        #[serde(rename = "anchoredSymbols")]
        anchored_symbols: Vec<serde_json::Value>,
        #[serde(rename = "lspDiagnosticsDelta")]
        lsp_diagnostics_delta: Vec<serde_json::Value>,
    },

    /// Test command finished with detailed per-test results.
    TestRunDetailed {
        command: String,
        framework: String, // "nextest" | "vitest" | "jest" | "pytest" | "unknown"
        passed_tests: u32,
        failed_tests: u32,
        newly_passing: Vec<String>,
        newly_failing: Vec<String>,
        coverage_delta: Option<common::coverage::CoverageDelta>,
        duration_ms: u64,
    },

    /// Tool profile (`/power on|off`) changed mid-thread.
    PowerModeToggled {
        previous: String,
        current: String,
        eager_tool_count: u32,
        deferred_tool_count: u32,
    },

    /// Agent loop interrupted before reaching its final turn.
    TurnInterrupted {
        reason: String, // "user_cancelled" | "budget_exceeded" | "provider_error" | etc.
        partial_tools: Vec<String>,
        iterations_completed: u32,
    },
}

#[cfg(test)]
mod recall_skill_variant_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recall_injected_serializes_with_camel_case_tag() {
        let event = AgentEvent::RecallInjected {
            memory_ids: vec!["m1".into(), "m2".into()],
            coverage_score: 0.82,
            escalation_chain: vec!["thread".into(), "repo".into()],
            dead_end_warning: false,
            budget_used_tokens: 1234,
            budget_limit_tokens: 4096,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "recallInjected");
        assert_eq!(v["coverageScore"], 0.82);
        assert_eq!(v["budgetUsedTokens"], 1234);
    }

    #[test]
    fn skill_activated_serializes() {
        let event = AgentEvent::SkillActivated {
            skill_id: "code-review".into(),
            source_path: "~/.klyntbot/skills/code-review".into(),
            trigger: "path_touch".into(),
            injected_tokens: 480,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "skillActivated");
        assert_eq!(v["skillId"], "code-review");
    }

    #[test]
    fn dead_end_warning_skill_reference_context_decision_serialize() {
        let _e1 = AgentEvent::DeadEndWarningSurfaced {
            approach_summary: "tried subprocess but env wrong".into(),
            prior_attempt_id: "att-1".into(),
            confidence: 0.7,
        };
        let _e2 = AgentEvent::SkillActivationConsidered {
            skill_id: "code-review".into(),
            score: 0.65,
            threshold: 0.5,
            accepted: true,
            decision_reason: "above threshold".into(),
        };
        let _e3 = AgentEvent::SkillReferenceLoaded {
            skill_id: "code-review".into(),
            reference: "examples.md".into(),
            tokens: 1200,
            load_kind: "on_demand".into(),
        };
        let _e4 = AgentEvent::ContextEngineDecision {
            included: vec!["soul".into(), "code-review-skill".into()],
            excluded: vec!["legacy-foo".into()],
            total_tokens: 5400,
            budget_used_pct: 0.45,
        };
        // Smoke: each constructs without panic; serialization tested above on representative ones.
    }
}

#[cfg(test)]
mod approval_sandbox_variant_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn approval_requested_carries_requires_user_input_field() {
        let event = AgentEvent::ApprovalRequested {
            request_id: "req-1".into(),
            tool: "bash".into(),
            args_hash: "abc123".into(),
            layer: "starlark".into(),
            rule_matched: Some("prefix git push".into()),
            mirror_history: None,
            sandbox_summary: "Seatbelt cwd-only".into(),
            requires_user_input: true,
            args: None,
            cwd: None,
            layer_reason: None,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "approvalRequested");
        assert_eq!(v["requiresUserInput"], true);
    }

    #[test]
    fn approval_resolved_carries_decided_by_field() {
        let event = AgentEvent::ApprovalResolved {
            request_id: "req-1".into(),
            decision: "allow_once".into(),
            decision_reason: "user clicked allow".into(),
            latency_ms: 8200,
            persisted_rule: None,
            decided_by: "user".into(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "approvalResolved");
        assert_eq!(v["decidedBy"], "user");
    }

    #[test]
    fn sandbox_policy_applied_serializes() {
        let event = AgentEvent::SandboxPolicyApplied {
            tool: "bash".into(),
            policy_summary: "seatbelt cwd-only".into(),
            policy_hash: "h123".into(),
            fallback_unsandboxed: false,
            fs_constraints: vec!["read /Users/jayden/Projects/Klynt/bot/**".into()],
            network_constraints: vec!["deny".into()],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "sandboxPolicyApplied");
        assert_eq!(v["fallbackUnsandboxed"], false);
    }
}

#[cfg(test)]
mod tool_provider_variant_tests {
    use super::*;

    #[test]
    fn provider_response_serializes() {
        let event = AgentEvent::ProviderResponse {
            latency_ms: 1234,
            usage: serde_json::json!({"prompt_tokens": 1000, "completion_tokens": 200}),
            cost_usd: 0.042,
            retries_used: 0,
            finish_reason: "stop".into(),
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "providerResponse");
        assert_eq!(v["latencyMs"], 1234);
    }

    #[test]
    fn mid_loop_compression_serializes() {
        let event = AgentEvent::MidLoopCompressionTriggered {
            before_tokens: 95000,
            after_tokens: 38000,
            messages_condensed: 47,
            regions: vec!["tool_results".into()],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "midLoopCompressionTriggered");
    }

    #[test]
    fn tool_call_stream_chunk_mcp_subcall_provider_request_smoke() {
        let _e1 = AgentEvent::ToolCallStreamChunk {
            tool: "bash".into(),
            chunk_kind: "stdout".into(),
            bytes: 4096,
            truncated: false,
        };
        let _e2 = AgentEvent::MCPSubcallTrace {
            server: "google-calendar".into(),
            tool: "list_events".into(),
            latency_ms: 230,
            bytes_returned: 1280,
            error: None,
        };
        let _e3 = AgentEvent::ProviderRequest {
            iteration: 3,
            model: "claude-sonnet-4-7".into(),
            prompt_tokens: 4500,
            max_tokens: 4096,
            attempt: 1,
        };
    }
}

#[cfg(test)]
mod coding_specific_variant_tests {
    use super::*;

    #[test]
    fn file_edit_with_symbols_serializes_with_phase1_stub_fields_empty() {
        let event = AgentEvent::FileEditWithSymbols {
            path: "crates/agent/src/events.rs".into(),
            op: "edit".into(),
            bytes: 1240,
            diff_full: "@@ ... @@".into(),
            anchored_symbols: vec![],
            lsp_diagnostics_delta: vec![],
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "fileEditWithSymbols");
        assert_eq!(v["anchoredSymbols"], serde_json::json!([]));
        assert_eq!(v["lspDiagnosticsDelta"], serde_json::json!([]));
    }

    #[test]
    fn test_run_detailed_serializes() {
        let event = AgentEvent::TestRunDetailed {
            command: "cargo nextest run -p agent".into(),
            framework: "nextest".into(),
            passed_tests: 42,
            failed_tests: 0,
            newly_passing: vec![],
            newly_failing: vec![],
            coverage_delta: None,
            duration_ms: 8200,
        };
        let v = serde_json::to_value(&event).unwrap();
        assert_eq!(v["type"], "testRunDetailed");
        assert_eq!(v["passed_tests"], 42);
    }

    #[test]
    fn power_mode_toggled_turn_interrupted_smoke() {
        let _e1 = AgentEvent::PowerModeToggled {
            previous: "curated".into(),
            current: "power".into(),
            eager_tool_count: 36,
            deferred_tool_count: 0,
        };
        let _e2 = AgentEvent::TurnInterrupted {
            reason: "user_cancelled".into(),
            partial_tools: vec!["bash".into()],
            iterations_completed: 2,
        };
    }
}
