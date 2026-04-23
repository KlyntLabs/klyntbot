use extism_pdk::*;

#[plugin_fn]
pub fn hello_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let name = args["name"].as_str().unwrap_or("world");
    Ok(format!("hello from wasm, {}!", name))
}

#[plugin_fn]
pub fn emit_test_event(_input: String) -> FnResult<String> {
    let payload = r#"{"hello":"from plugin"}"#;
    let req = format!(r#"{{"kind":"hello_emitted","payload":{}}}"#, payload);
    let resp: String = unsafe { agent_emit_event(&req)? };
    Ok(resp)
}

#[host_fn("klyntbot")]
extern "ExtismHost" {
    fn agent_emit_event(input: &str) -> String;
}
