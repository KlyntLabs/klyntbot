//! Integration: verify the JSON body produced by AnthropicNativeProvider
//! contains cache_control on the right blocks given various breakpoint
//! configurations.

use providers::{
    adapters::anthropic_native::AnthropicNativeProvider, CacheAnchor, CacheBreakpoint, CacheTtl,
    ChatParams, Message,
};

fn provider() -> AnthropicNativeProvider {
    AnthropicNativeProvider::new_for_test(false)
}

fn openai_tool(name: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": name,
            "description": "test tool",
            "parameters": {}
        }
    })
}

fn body_for(
    messages: &[Message],
    tools: Option<&[serde_json::Value]>,
    bps: &[CacheBreakpoint],
) -> serde_json::Value {
    provider().build_request_body(
        messages,
        tools,
        &ChatParams::new("claude-3-5-sonnet"),
        false,
        bps,
    )
}

#[test]
fn three_breakpoints_apply_three_cache_controls() {
    let messages = vec![
        Message::System {
            content: "sys".into(),
        },
        Message::user("u0"),
        Message::user("u1"),
        Message::user("u2"),
    ];
    let tools = vec![openai_tool("echo")];
    let bps = vec![
        CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        },
    ];
    let body = body_for(&messages, Some(&tools), &bps);

    // Last system block has cache_control 1h
    let sys_blocks = body.get("system").unwrap().as_array().unwrap();
    let last_sys = sys_blocks.last().unwrap();
    assert_eq!(
        last_sys
            .get("cache_control")
            .unwrap()
            .get("ttl")
            .and_then(|t| t.as_str()),
        Some("1h"),
    );

    // Last tool has cache_control 1h
    let tools_arr = body.get("tools").unwrap().as_array().unwrap();
    let last_tool = tools_arr.last().unwrap();
    assert_eq!(
        last_tool
            .get("cache_control")
            .unwrap()
            .get("ttl")
            .and_then(|t| t.as_str()),
        Some("1h"),
    );

    // Verify SOME message has the ephemeral marker
    let msgs_arr = body.get("messages").unwrap().as_array().unwrap();
    let has_ephemeral = msgs_arr.iter().any(|m| {
        let content = m.get("content");
        match content {
            Some(serde_json::Value::Array(blocks)) => blocks.iter().any(|b| {
                b.get("cache_control")
                    .and_then(|cc| cc.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("ephemeral")
                    && b.get("cache_control")
                        .and_then(|cc| cc.get("ttl"))
                        .is_none()
            }),
            _ => false,
        }
    });
    assert!(
        has_ephemeral,
        "expected an ephemeral cache_control on a message block"
    );
}

#[test]
fn empty_breakpoints_with_legacy_flag_marks_only_last_system() {
    let provider = AnthropicNativeProvider::new_for_test(true);
    let body = provider.build_request_body(
        &[
            Message::System {
                content: "sys".into(),
            },
            Message::user("hi"),
        ],
        None,
        &ChatParams::new("claude-3-5-sonnet"),
        false,
        &[],
    );
    let sys_blocks = body.get("system").unwrap().as_array().unwrap();
    assert!(sys_blocks[0].get("cache_control").is_some());
}

#[test]
fn five_breakpoints_drop_to_trailing_four() {
    let messages = vec![
        Message::System {
            content: "sys".into(),
        },
        Message::user("u0"),
        Message::user("u1"),
        Message::user("u2"),
        Message::user("u3"),
        Message::user("u4"),
    ];
    let tools = vec![openai_tool("t")];
    let bps = vec![
        CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(3),
            ttl: CacheTtl::Ephemeral,
        },
        CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(4),
            ttl: CacheTtl::Ephemeral,
        },
    ];
    let body = body_for(&messages, Some(&tools), &bps);

    let mut total_cc = 0usize;
    if let Some(sys) = body.get("system").and_then(|s| s.as_array()) {
        for b in sys {
            if b.get("cache_control").is_some() {
                total_cc += 1;
            }
        }
    }
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for b in tools {
            if b.get("cache_control").is_some() {
                total_cc += 1;
            }
        }
    }
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            if let Some(serde_json::Value::Array(blocks)) = m.get("content") {
                for b in blocks {
                    if b.get("cache_control").is_some() {
                        total_cc += 1;
                    }
                }
            }
        }
    }
    assert_eq!(total_cc, 4, "expected exactly 4 cache_control markers");
}
