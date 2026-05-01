//! JSON schema loader for hook event validation.
//!
//! Schemas are embedded at compile time via `include_str!` from
//! `crates/klynt-hooks/schema/generated/`.

/// Load the input JSON schema for a given event name.
pub fn load_input_schema(event_name: &str) -> Option<&'static str> {
    match event_name {
        "pre_tool_use" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/generated/pre-tool-use.command.input.schema.json"))),
        "post_tool_use" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/generated/post-tool-use.command.input.schema.json"))),
        "session_start" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/generated/session-start.command.input.schema.json"))),
        "user_prompt_submit" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/generated/user-prompt-submit.command.input.schema.json"))),
        "stop" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/schema/generated/stop.command.input.schema.json"))),
        _ => None,
    }
}
