//! Cache-breakpoint placement policies.
//!
//! See docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md
//! Appendix B for the compression-survivability analysis.

use providers::{CacheAnchor, CacheBreakpoint, CacheTtl, Message};
use serde_json::Value;

use super::mid_loop_compressor::MidLoopCompressor;

/// Default placement policy. Emits 2–3 breakpoints per call:
///
/// 1. `LastSystem` with `Persistent` TTL — system prompt durable across the session.
/// 2. `LastTool`   with `Persistent` TTL — tool definitions durable when present.
/// 3. `MessageIndex(frontier - 1)` with `Ephemeral` TTL — anchored at the
///    boundary of the compression mutation zone. The cached prefix
///    includes the mutation zone, so a compression event invalidates this
///    entry as a full match (Anthropic's longest-prefix-match still gives
///    a partial cache hit on the system+tools prefix afterward). Within a
///    compression-free run of turns it accelerates every call. Ephemeral
///    is correct because (a) the cache is invalidated by compression
///    anyway, and (b) the 5-min TTL matches typical ReAct-burst cadence.
pub fn compression_aware_default(
    messages: &[Message],
    tools: Option<&[Value]>,
    compressor: &MidLoopCompressor,
) -> Vec<CacheBreakpoint> {
    let mut bps = Vec::with_capacity(3);

    // 1. System prompt — durable, worth Persistent
    if messages.iter().any(|m| m.is_system()) {
        bps.push(CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        });
    }

    // 2. Tool definitions — durable when present
    if matches!(tools, Some(t) if !t.is_empty()) {
        bps.push(CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        });
    }

    // 3. Pre-frontier prefix — accelerates intra-window turns
    let frontier = compressor.frontier_index(messages);
    if let Some(idx) = frontier.checked_sub(1) {
        bps.push(CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(idx),
            ttl: CacheTtl::Ephemeral,
        });
    }

    bps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_compressor() -> MidLoopCompressor {
        MidLoopCompressor::new(Arc::new(context_engine::CharTokenCounter), 10_000)
    }

    fn sys() -> Message {
        Message::System {
            content: "sys".into(),
        }
    }

    fn user(t: &str) -> Message {
        Message::user(t)
    }

    fn assistant(t: &str) -> Message {
        Message::Assistant {
            content: Some(t.into()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    #[test]
    fn empty_conversation_emits_only_last_system() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let bps = compression_aware_default(&messages, None, &compressor);
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].anchor, CacheAnchor::LastSystem);
        assert_eq!(bps[0].ttl, CacheTtl::Persistent);
    }

    #[test]
    fn empty_conversation_with_tools_emits_two_persistent() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let tools = vec![serde_json::json!({"name": "echo"})];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert_eq!(bps.len(), 2);
        assert!(matches!(bps[0].anchor, CacheAnchor::LastSystem));
        assert!(matches!(bps[1].anchor, CacheAnchor::LastTool));
        assert_eq!(bps[0].ttl, CacheTtl::Persistent);
        assert_eq!(bps[1].ttl, CacheTtl::Persistent);
    }

    #[test]
    fn long_conversation_emits_three_breakpoints() {
        let compressor = make_compressor();
        let mut messages = vec![sys()];
        for i in 0..12 {
            messages.push(user(&format!("u{i}")));
            messages.push(assistant(&format!("a{i}")));
        }
        let tools = vec![serde_json::json!({"name": "echo"})];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert_eq!(bps.len(), 3);
        let frontier = compressor.frontier_index(&messages);
        assert!(frontier > 0);
        assert_eq!(bps[2].anchor, CacheAnchor::MessageIndex(frontier - 1));
        assert_eq!(bps[2].ttl, CacheTtl::Ephemeral);
    }

    #[test]
    fn no_tools_means_no_last_tool_breakpoint() {
        let compressor = make_compressor();
        let messages = vec![sys(), user("u1")];
        let bps = compression_aware_default(&messages, None, &compressor);
        assert!(!bps
            .iter()
            .any(|b| matches!(b.anchor, CacheAnchor::LastTool)));
    }

    #[test]
    fn empty_tools_array_treated_as_no_tools() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let tools: Vec<Value> = vec![];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert!(!bps
            .iter()
            .any(|b| matches!(b.anchor, CacheAnchor::LastTool)));
    }

    #[test]
    fn no_system_messages_means_no_last_system_breakpoint() {
        let compressor = make_compressor();
        let messages = vec![user("hi")];
        let bps = compression_aware_default(&messages, None, &compressor);
        assert!(!bps
            .iter()
            .any(|b| matches!(b.anchor, CacheAnchor::LastSystem)));
    }
}
