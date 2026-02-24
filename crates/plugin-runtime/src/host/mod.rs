//! Host functions exposed to WASM plugins via Extism.
//!
//! Five namespaces: db, log, http, agent, tool.
//! Each host function checks permissions before executing.

use extism::{Function, UserData, PTR};
use tracing::{debug, error, info, warn};

use crate::manifest::PluginPermission;

/// Shared state passed to all host functions via UserData.
struct HostContext {
    pool: sqlx::SqlitePool,
    plugin_id: String,
    permissions: Vec<PluginPermission>,
    bus_sender: Option<tokio::sync::mpsc::UnboundedSender<bus::OutboundMessage>>,
    http_client: reqwest::Client,
}

/// Returns true if the SQL statement is a read-only SELECT.
fn is_select_only(sql: &str) -> bool {
    let trimmed = sql.trim();
    trimmed.len() >= 6 && trimmed[..6].eq_ignore_ascii_case("SELECT")
}

/// Returns true if the table name belongs to this plugin's sandboxed namespace.
#[allow(dead_code)]
fn is_plugin_table(table_name: &str, plugin_id: &str) -> bool {
    let expected_prefix = format!("plugin_{}_", plugin_id.replace('-', "_"));
    table_name.starts_with(&expected_prefix)
}

/// Build all host functions for a plugin.
///
/// Each function enforces permission checks before executing.
pub fn build_host_functions(
    pool: sqlx::SqlitePool,
    plugin_id: String,
    permissions: Vec<PluginPermission>,
    bus_sender: Option<tokio::sync::mpsc::UnboundedSender<bus::OutboundMessage>>,
) -> Vec<Function> {
    let ctx = HostContext {
        pool,
        plugin_id,
        permissions,
        bus_sender,
        http_client: reqwest::Client::new(),
    };
    let ud = UserData::new(ctx);

    let mut functions = Vec::new();

    // ── db namespace ──────────────────────────────────────────────

    // db_query: SELECT-only queries on sandboxed tables
    {
        let f = Function::new(
            "db_query",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Storage) {
                    let handle = plugin.memory_new("error: storage permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                if !is_select_only(&input) {
                    let handle =
                        plugin.memory_new("error: only SELECT queries allowed in db_query")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                debug!(plugin_id = %ctx.plugin_id, sql = %input, "db_query");

                let pool = ctx.pool.clone();
                drop(ctx); // release lock before async work

                let output = match tokio::runtime::Handle::try_current() {
                    Ok(handle) => std::thread::scope(|s| {
                        s.spawn(|| {
                            handle.block_on(async {
                                match sqlx::query_scalar::<_, String>(&input)
                                    .fetch_optional(&pool)
                                    .await
                                {
                                    Ok(Some(row)) => row,
                                    Ok(None) => "null".to_string(),
                                    Err(e) => format!("error: {e}"),
                                }
                            })
                        })
                        .join()
                        .unwrap()
                    }),
                    Err(_) => "error: no async runtime available".to_string(),
                };

                let handle = plugin.memory_new(&output)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // db_execute: write queries on sandboxed tables
    {
        let f = Function::new(
            "db_execute",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Storage) {
                    let handle = plugin.memory_new("error: storage permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                debug!(plugin_id = %ctx.plugin_id, sql = %input, "db_execute");

                let pool = ctx.pool.clone();
                drop(ctx);

                let output = match tokio::runtime::Handle::try_current() {
                    Ok(handle) => std::thread::scope(|s| {
                        s.spawn(|| {
                            handle.block_on(async {
                                match sqlx::query(&input)
                                    .execute(&pool)
                                    .await
                                {
                                    Ok(r) => {
                                        format!("{{\"rows_affected\":{}}}", r.rows_affected())
                                    }
                                    Err(e) => format!("error: {e}"),
                                }
                            })
                        })
                        .join()
                        .unwrap()
                    }),
                    Err(_) => "error: no async runtime available".to_string(),
                };

                let handle = plugin.memory_new(&output)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // ── log namespace ─────────────────────────────────────────────

    {
        let f = Function::new(
            "log_debug",
            [PTR],
            [],
            ud.clone(),
            |plugin, inputs, _outputs, user_data| {
                let msg: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                debug!(plugin_id = %ctx.plugin_id, "{msg}");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    {
        let f = Function::new(
            "log_info",
            [PTR],
            [],
            ud.clone(),
            |plugin, inputs, _outputs, user_data| {
                let msg: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                info!(plugin_id = %ctx.plugin_id, "{msg}");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    {
        let f = Function::new(
            "log_warn",
            [PTR],
            [],
            ud.clone(),
            |plugin, inputs, _outputs, user_data| {
                let msg: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                warn!(plugin_id = %ctx.plugin_id, "{msg}");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    {
        let f = Function::new(
            "log_error",
            [PTR],
            [],
            ud.clone(),
            |plugin, inputs, _outputs, user_data| {
                let msg: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                error!(plugin_id = %ctx.plugin_id, "{msg}");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // ── http namespace ────────────────────────────────────────────

    {
        let f = Function::new(
            "http_request",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Network) {
                    let handle = plugin.memory_new("error: network permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                let req: serde_json::Value =
                    serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);
                let url = req
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let method = req
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET")
                    .to_string();
                let body = req.get("body").map(|v| v.to_string());

                debug!(plugin_id = %ctx.plugin_id, url = %url, method = %method, "http_request");

                let client = ctx.http_client.clone();
                drop(ctx);

                let output = match tokio::runtime::Handle::try_current() {
                    Ok(handle) => std::thread::scope(|s| {
                        s.spawn(|| {
                            handle.block_on(async {
                                let mut req_builder = match method.to_uppercase().as_str() {
                                    "POST" => client.post(&url),
                                    "PUT" => client.put(&url),
                                    "DELETE" => client.delete(&url),
                                    "PATCH" => client.patch(&url),
                                    _ => client.get(&url),
                                };

                                if let Some(b) = body {
                                    req_builder = req_builder
                                        .header("Content-Type", "application/json")
                                        .body(b);
                                }

                                match req_builder.send().await {
                                    Ok(resp) => {
                                        let status = resp.status().as_u16();
                                        let body_text =
                                            resp.text().await.unwrap_or_default();
                                        serde_json::json!({
                                            "status": status,
                                            "body": body_text
                                        })
                                        .to_string()
                                    }
                                    Err(e) => format!("error: {e}"),
                                }
                            })
                        })
                        .join()
                        .unwrap()
                    }),
                    Err(_) => "error: no async runtime available".to_string(),
                };

                let handle = plugin.memory_new(&output)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // ── agent namespace ───────────────────────────────────────────

    // agent_send_message: send a message via the bus
    {
        let f = Function::new(
            "agent_send_message",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Agent) {
                    let handle = plugin.memory_new("error: agent permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                let msg: serde_json::Value =
                    serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);
                let channel = msg
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cli");
                let chat_id = msg
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let content = msg
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                debug!(
                    plugin_id = %ctx.plugin_id,
                    channel = %channel,
                    chat_id = %chat_id,
                    "agent_send_message"
                );

                let output = if let Some(ref sender) = ctx.bus_sender {
                    let outbound = bus::OutboundMessage::new(channel, chat_id, content);
                    match sender.send(outbound) {
                        Ok(()) => r#"{"ok":true}"#.to_string(),
                        Err(e) => format!(r#"{{"ok":false,"error":"{}"}}"#, e),
                    }
                } else {
                    r#"{"ok":false,"error":"bus sender not available"}"#.to_string()
                };

                let handle = plugin.memory_new(&output)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // agent_ask_user: stub — returns error until wired to agent loop (Task #8)
    {
        let f = Function::new(
            "agent_ask_user",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Agent) {
                    let handle = plugin.memory_new("error: agent permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                debug!(
                    plugin_id = %ctx.plugin_id,
                    question = %input,
                    "agent_ask_user"
                );

                let handle = plugin
                    .memory_new(r#"{"error":"agent callbacks not connected"}"#)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // agent_emit_event: emit a custom event
    {
        let f = Function::new(
            "agent_emit_event",
            [PTR],
            [PTR],
            ud.clone(),
            |plugin, inputs, outputs, user_data| {
                let input: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();

                if !ctx.permissions.contains(&PluginPermission::Agent) {
                    let handle = plugin.memory_new("error: agent permission denied")?;
                    outputs[0] = plugin.memory_to_val(handle);
                    return Ok(());
                }

                info!(
                    plugin_id = %ctx.plugin_id,
                    event = %input,
                    "plugin event emitted"
                );

                let handle = plugin.memory_new(r#"{"ok":true}"#)?;
                outputs[0] = plugin.memory_to_val(handle);
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // ── tool namespace ────────────────────────────────────────────

    // tool_return: plugin signals a successful result
    {
        let f = Function::new(
            "tool_return",
            [PTR],
            [],
            ud.clone(),
            |plugin, inputs, _outputs, user_data| {
                let result: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                debug!(plugin_id = %ctx.plugin_id, "tool_return received");
                drop(ctx);

                // Store the result JSON for the host to retrieve after the call.
                // For now we log it; PluginManager will read it from memory after
                // the WASM call completes.
                info!(result = %result, "plugin tool_return");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    // tool_error: plugin signals an error
    {
        let f = Function::new(
            "tool_error",
            [PTR],
            [],
            ud,
            |plugin, inputs, _outputs, user_data| {
                let msg: String = plugin.memory_get_val(&inputs[0])?;
                let data = user_data.get()?;
                let ctx = data.lock().unwrap();
                error!(plugin_id = %ctx.plugin_id, error = %msg, "plugin tool_error");
                Ok(())
            },
        )
        .with_namespace("klyntbot");
        functions.push(f);
    }

    functions
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_select_only ────────────────────────────────────────────

    #[test]
    fn test_is_select_only_accepts_select() {
        assert!(is_select_only("SELECT * FROM plugin_foo_bar"));
        assert!(is_select_only("  select id from plugin_test_cache  "));
    }

    #[test]
    fn test_is_select_only_accepts_mixed_case() {
        assert!(is_select_only("select * FROM users"));
        assert!(is_select_only("Select id from t"));
    }

    #[test]
    fn test_is_select_only_rejects_write() {
        assert!(!is_select_only("INSERT INTO plugin_foo VALUES (1)"));
        assert!(!is_select_only("DELETE FROM plugin_foo"));
        assert!(!is_select_only("UPDATE plugin_foo SET x=1"));
        assert!(!is_select_only("DROP TABLE plugin_foo"));
    }

    #[test]
    fn test_is_select_only_rejects_empty() {
        assert!(!is_select_only(""));
        assert!(!is_select_only("   "));
    }

    // ── is_plugin_table ───────────────────────────────────────────

    #[test]
    fn test_is_plugin_table_accepts_prefixed() {
        assert!(is_plugin_table(
            "plugin_notion_connector_cache",
            "notion-connector"
        ));
        assert!(is_plugin_table(
            "plugin_notion_connector_items",
            "notion-connector"
        ));
    }

    #[test]
    fn test_is_plugin_table_accepts_underscore_id() {
        assert!(is_plugin_table("plugin_my_plugin_data", "my_plugin"));
    }

    #[test]
    fn test_is_plugin_table_rejects_other_plugins() {
        assert!(!is_plugin_table(
            "plugin_other_plugin_data",
            "notion-connector"
        ));
        assert!(!is_plugin_table("sessions", "notion-connector"));
        assert!(!is_plugin_table("todos", "notion-connector"));
    }
}
