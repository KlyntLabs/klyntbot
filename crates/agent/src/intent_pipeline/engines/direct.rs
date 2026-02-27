//! DirectEngine — handles Direct execution mode (single LLM call, no tools).
//!
//! Ported from `execution/direct.rs` but returns `EngineResult` instead of
//! `DirectOutcome`. Escalates to Reactive if the LLM generates tool calls.

// Implementation in Task 9.
