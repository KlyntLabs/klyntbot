# Coding Memory — Phase 4 Implementation Summary

## Overview

Phase 4 turns Phase-1 recall stubs into a working read pipeline. The Distiller (Phase 3) writes facts and episodes; Phase 4 makes them retrievable via MCP tools, passive markdown injection, and C3 escalation-aware retrieval.

## What landed in Phase 4

- **7 active recall MCP tools** — `recall_index`, `recall_timeline`, `recall_fetch`, `check_dead_ends`, `recall_facts_as_of`, `recall_change_history`, `recall_decision_points`
- **Passive markdown injection** — `klyntbot-hook context --session-start` and `--user-prompt-submit` emit truncated markdown to stdout for Claude Code's `additionalContext`
- **C3 retrieval skills** — closed set of 5 skills (`QueryRewriter`, `QueryDecomposer`, `EvidenceFocuser`, `RawEventEscalator`, `CausalContextExpander`) with EMA effectiveness tracking
- **Dead-end warning block** — Tier B1 counterfactual match surfaces a warning when the user prompt resembles a prior failed attempt
- **Recall-invocation telemetry** — every recall call persists a row to `recall_invocations` with query, coverage score, skill used, latency, and result ids
- **Workbench panels** — Recall Tool Log panel + Session Replay recall overlay

## How recall flows

```
┌─────────────────┐     ┌─────────────────────┐     ┌─────────────────┐
│ klyntbot-hook   │────▶│ IngestDaemon        │────▶│ RecallOpHandler │
│ context ...     │     │ (op frame)          │     │ (renderers)     │
└─────────────────┘     └─────────────────────┘     └─────────────────┘
        │                                               │
        │ passive markdown to stdout                    │ markdown response
        ▼                                               ▼
┌─────────────────┐                           ┌─────────────────┐
│ Claude Code     │                           │ Workbench UI    │
│ additionalCtx   │                           │ (Session Replay)│
└─────────────────┘                           └─────────────────┘

┌─────────────────┐     ┌─────────────────────┐     ┌─────────────────┐
│ MCP client      │────▶│ ToolRegistryBridge  │────▶│ CodingMemory    │
│ (Cursor, etc.)  │     │                     │     │ McpTool (×8)    │
└─────────────────┘     └─────────────────────┘     └─────────────────┘
                                                              │
                        ┌─────────────────────────────────────┘
                        ▼
              ┌─────────────────┐
              │ CodingRecallSvc │
              └────────┬────────┘
                       │
     ┌─────────────────┼─────────────────┐
     │                 │                 │
     ▼                 ▼                 ▼
┌─────────┐    ┌────────────┐    ┌─────────────┐
│ UMS     │───▶│ Probe      │───▶│ SkillRegistry│
│ retrieve│    │ coverage   │    │ escalate     │
└─────────┘    └────────────┘    └─────────────┘
```

## MCP tool surface

| Tool | Args (camelCase) | Returns |
|------|-----------------|---------|
| `recall_index` | `query`, `repo?`, `kinds?`, `days?`, `limit?` | `RecallIndexResponse` |
| `recall_timeline` | `ids?` / `query?`, `repo?`, `days?` | `TimelineEntry[]` |
| `recall_fetch` | `ids`, `includeProvenance?`, `includeCausalGraph?` | `FullEntry[]` |
| `check_dead_ends` | `approach`, `repo?` | `DeadEndResponse` |
| `recall_facts_as_of` | `subject`, `predicate`, `asOf` | `FactsAsOfResponse` |
| `recall_change_history` | `subject`, `predicate`, `repo?` | `ChangeHistoryResponse` |
| `recall_decision_points` | `domain?`, `repo?`, `limit?` | `DecisionPointsResponse` |
| `trace_causes` | `subject` | **Stub** — Phase 6 |

All tools are registered into the agent's `ToolRegistry` at `AppCore` init via `CodingMemoryMcpTool` wrappers.

## Token budgets and truncation invariant

- `SESSION_START_BUDGET_TOKENS = 800`
- `USER_PROMPT_BUDGET_TOKENS = 1500`

Both renderers truncate section-by-section using a pluggable `TokenBudgeter`. Default: `HeuristicBudgeter` (`chars / 4`). `TiktokenBudgeter` is available as a fallback. The renderer always returns `count(output) ≤ budget + 1` (the `+1` accounts for the ellipsis token).

## C3 retrieval skills — the closed set of 5

| Skill | Tier | What it does |
|-------|------|-------------|
| `QueryRewriter` | `DeepThink` | PRF-style multi-query expansion (3 rewrites) + RRF merge |
| `QueryDecomposer` | `DeepThink` | Splits compound queries, per-clause retrieve, RRF merge |
| `EvidenceFocuser` | `DeepThink` | Token-cosine rerank: top-20 → top-5 |
| `RawEventEscalator` | `Ultra` | Looks up provenance ids in `ingest_event_log` |
| `CausalContextExpander` | `Ultra` | **Stub** — walks causal edges (Phase 6) |

The `RetrievalSkillRegistry` selector:
1. Filters skills by tier rank ≤ active budget tier
2. Sorts by EMA effectiveness descending
3. Tries each until one succeeds (coverage_after > threshold)
4. Publishes `DomainEvent::RetrievalSkillApplied` after each attempt

EMA update: `next = 0.9 * prev + 0.1 * value` where value is `1.0` on success, `0.0` on failure.

## What is still stubbed

- `trace_causes` — causal graph walking; needs `memory_causal_edges` population (Phase 6)
- `CausalContextExpander` — same dependency
- Causal edge population in the Distiller — currently no edges are written

## Phase-5 hand-off

Reforge will start consuming `recall_invocations` for ineffective-memory signals:
- Low coverage_score + no skill success → "memory gap" signal
- Repeated dead-end warnings → "intervention needed" signal
- EMA trending down for a skill → "skill needs tuning" signal

The telemetry table is the bridge between Phase-4 retrieval and Phase-5 reflection.
