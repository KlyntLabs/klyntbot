use extism_pdk::*;

#[plugin_fn]
pub fn hello_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let name = args["name"].as_str().unwrap_or("world");
    Ok(format!("hello from wasm, {}!", name))
}
