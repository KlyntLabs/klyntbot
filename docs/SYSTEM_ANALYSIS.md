# Klyntbot System Analysis

> **Last updated:** 2026-03-09
> **Tested by:** Claude Code automated browser testing (Chrome on localhost:1420)

## System Health Score: 90/100

| Category | Score | Details |
|----------|-------|---------|
| Cognitive Memory | 18/20 | Multi-fact extraction, cross-session recall, fact update/consolidation, question filtering — all pass. FSRS scheduling active. |
| Agent Runtime | 18/20 | Direct/Reactive modes, multi-agent delegation, chain-of-thought planning, intent classification — all pass. |
| Tool Execution | 16/20 | Task, Finance, Notes, Productivity, Cron tools all functional. Web search requires external API. Distraction tool requires macOS accessibility. |
| Coaching Pipeline | 18/20 | Signal accumulation → trigger evaluation → LLM reasoning → intervention delivery → user feedback loop — fully wired and tested. |
| Desktop UI | 10/10 | Real-time event streaming, execution panel, debug dashboard, coaching feedback UI — all functional. |
| Session & Persistence | 10/10 | Multi-turn context, session isolation, concurrent stress test passed. |

## Test Results

**18/20 scenarios pass** (2 skipped — require macOS accessibility / API key manipulation)

See: [System Test Scenarios](plans/2026-03-08-system-test-scenarios.md)

### Recently Verified (2026-03-09)

| Component | Status | Evidence |
|-----------|--------|----------|
| Coaching UserSituation | Working | Real data from productivity repos (Energy 85%, Focus 0%, Distraction 60%, etc.) |
| Coaching Signal Accumulator | Working | 5+ DistractionDetected signals accumulated in 30min window |
| Coaching Trigger Evaluation | Working | `distraction_streak` (≥3 signals), `context_switch_overload` (>10 switches) both fired |
| Coaching LLM Reasoner | Working | Generated contextual coaching messages via LLM provider |
| Coaching Intervention Router | Working | Rate limiting: 2/3 hourly, 2/10 daily. Cooldowns active (231s, 213s) |
| Coaching Feedback UI | Working | Helpful → Accept 100%/Effect 100%; Dismiss → Accept 0%/Effect -50% |
| Coaching Strategy Feedback | Working | Both `distraction_streak` and `context_switch_overload` strategies tracked |
| Cognitive Memory Recall | Working | Full user profile recalled across sessions (name, occupation, preferences, projects) |
| Agent Classification | Working | Direct mode for simple queries, Reactive for tool-requiring tasks |
| Multi-Agent Delegation | Working | General → Task + Finance delegation chains |

## Architecture Layers Verified

```
L0: common          ✅ Error types, message roles, session keys
L1: config, bus     ✅ DomainEventBus (broadcast 256), config loading
L2: storage         ✅ SQLite repos, vector store (LanceDB), FSRS scheduling
L3: providers       ✅ LLM streaming, cognitive provider, session management
L4: features        ✅ todo, finance, notes, productivity, coaching — all operational
L5: agent           ✅ AgentRuntime, IntentAnalyzer, ExecutionRouter, CostTracker
L6: mcp             ✅ MCP tool integration (Google Calendar)
L7: app-core        ✅ Shared handlers, Tauri commands, dev server (port 3456)
```

## Known Gaps

| Gap | Impact | Mitigation |
|-----|--------|------------|
| T10: Productivity Context | Cannot test activity tracker | Requires macOS accessibility permissions |
| T15: Error Recovery | Cannot test circuit breaker | Requires API key manipulation |
| Coaching SSE in browser dev mode | Interventions don't push via SSE | Polling fallback every 5s works |
| Weekly reflection | Needs 7+ days of data | Can test manually via debug page button |

## Bugs Found & Fixed This Session

1. **Coaching UserSituation was hardcoded** — `compute_situation()` existed but was never called. Fixed: `build_situation_inputs()` queries real productivity/task data, `spawn_situation_recompute()` runs every 2min.
2. **Strategy Feedback field mismatch** — Frontend `StrategyFeedback` interface had wrong field names (`triggerName` vs `strategyType`). Fixed: aligned with backend `StrategyFeedbackResponse`.
3. **Coaching tab not auto-refreshing** — `useQuery` only fetches once. Fixed: added 5s polling via `useEffect` + `setInterval`.
4. **No coaching feedback UI** — No way for users to respond to interventions. Fixed: added `coaching_pending_interventions` query + `coaching_submit_feedback` mutation + Active Interventions card UI with Helpful/Dismiss/Stop buttons.
