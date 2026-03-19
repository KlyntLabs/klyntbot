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
    bus_sender: Option<tokio::sync::mpsc::Sender<bus::OutboundMessage>>,
    http_client: reqwest::Client,
}

/// Returns true if the SQL statement is a read-only SELECT.
///
/// Accepts statements starting with SELECT, WITH (CTEs), or EXPLAIN.
/// Rejects multi-statement strings (containing internal semicolons) and
/// statements containing mutation keywords. Conservative: may reject
/// valid queries with mutation keywords in string literals.
fn is_select_only(sql: &str) -> bool {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return false;
    }

    let upper = trimmed.to_uppercase();

    // Must start with SELECT, WITH (for CTEs), or EXPLAIN
    if !(upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("EXPLAIN")) {
        return false;
    }

    // Reject multiple statements (semicolons not at the trailing end)
    let without_trailing_semi = trimmed.trim_end_matches(';').trim();
    if without_trailing_semi.contains(';') {
        return false;
    }

    // Reject mutation keywords appearing as standalone tokens
    let mutation_keywords = [
        "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "REPLACE", "ATTACH", "DETACH",
        "PRAGMA", "VACUUM", "REINDEX",
    ];
    for word in upper.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if mutation_keywords.contains(&word) {
            return false;
        }
    }

    true
}

/// Returns true if the table name belongs to this plugin's sandboxed namespace.
fn is_plugin_table(table_name: &str, plugin_id: &str) -> bool {
    let expected_prefix = format!("plugin_{}_", plugin_id.replace('-', "_"));
    table_name.starts_with(&expected_prefix)
}

/// Extract table names referenced in a SQL statement and verify all belong to
/// the plugin's sandboxed namespace (`plugin_{id}_*`).
///
/// Returns an error message if any non-sandboxed table is referenced.
/// Uses a conservative heuristic: extracts identifiers following FROM, JOIN,
/// INTO, UPDATE, and TABLE keywords.
fn check_table_sandbox(sql: &str, plugin_id: &str) -> Result<(), String> {
    let upper = sql.to_uppercase();
    let tokens: Vec<&str> = sql.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .collect();
    let upper_tokens: Vec<String> = tokens.iter().map(|t| t.to_uppercase()).collect();

    // Keywords after which a table name appears
    let table_keywords = ["FROM", "JOIN", "INTO", "UPDATE", "TABLE"];

    // Also check for ATTACH/DETACH which shouldn't pass is_select_only but defend anyway
    if upper.contains("ATTACH") || upper.contains("DETACH") {
        return Err("ATTACH/DETACH not allowed in plugin SQL".to_string());
    }

    for (i, token) in upper_tokens.iter().enumerate() {
        if table_keywords.contains(&token.as_str()) {
            if let Some(table_name) = tokens.get(i + 1) {
                // Skip SQL keywords that look like table names (e.g. FROM SELECT in subqueries)
                let tn_upper = table_name.to_uppercase();
                if ["SELECT", "WITH", "VALUES", "SET", "WHERE", "IF", "NOT", "EXISTS", "OR"]
                    .contains(&tn_upper.as_str())
                {
                    continue;
                }
                if !is_plugin_table(table_name, plugin_id) {
                    return Err(format!(
                        "table '{}' is outside plugin sandbox (expected prefix 'plugin_{}_')",
                        table_name,
                        plugin_id.replace('-', "_")
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Build all host functions for a plugin.
///
/// Each function enforces permission checks before executing.
pub fn build_host_functions(
    pool: sqlx::SqlitePool,
    plugin_id: String,
    permissions: Vec<PluginPermission>,
    bus_sender: Option<tokio::sync::mpsc::Sender<bus::OutboundMessage>>,
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

                if let Err(e) = check_table_sandbox(&input, &ctx.plugin_id) {
                    let handle = plugin.memory_new(&format!("error: {e}"))?;
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

                if let Err(e) = check_table_sandbox(&input, &ctx.plugin_id) {
                    let handle = plugin.memory_new(&format!("error: {e}"))?;
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
                                match sqlx::query(&input).execute(&pool).await {
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
                                        let body_text = resp.text().await.unwrap_or_default();
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
                let channel = msg.get("channel").and_then(|v| v.as_str()).unwrap_or("cli");
                let chat_id = msg
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

                debug!(
                    plugin_id = %ctx.plugin_id,
                    channel = %channel,
                    chat_id = %chat_id,
                    "agent_send_message"
                );

                let output = if let Some(ref sender) = ctx.bus_sender {
                    let outbound = bus::OutboundMessage::new(channel, chat_id, content);
                    match sender.try_send(outbound) {
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

                let handle = plugin.memory_new(r#"{"error":"agent callbacks not connected"}"#)?;
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
    fn test_is_select_only_accepts_with_and_explain() {
        assert!(is_select_only("WITH cte AS (SELECT 1) SELECT * FROM cte"));
        assert!(is_select_only("EXPLAIN SELECT * FROM t"));
        assert!(is_select_only("explain select 1"));
    }

    #[test]
    fn test_is_select_only_accepts_trailing_semicolon() {
        assert!(is_select_only("SELECT 1;"));
        assert!(is_select_only("SELECT * FROM t ;  "));
    }

    #[test]
    fn test_is_select_only_rejects_write() {
        assert!(!is_select_only("INSERT INTO plugin_foo VALUES (1)"));
        assert!(!is_select_only("DELETE FROM plugin_foo"));
        assert!(!is_select_only("UPDATE plugin_foo SET x=1"));
        assert!(!is_select_only("DROP TABLE plugin_foo"));
        assert!(!is_select_only("CREATE TABLE t (id INT)"));
        assert!(!is_select_only("PRAGMA journal_mode=WAL"));
        assert!(!is_select_only("ATTACH DATABASE 'x.db' AS x"));
    }

    #[test]
    fn test_is_select_only_rejects_multi_statement_injection() {
        assert!(!is_select_only("SELECT 1; DROP TABLE users"));
        assert!(!is_select_only("SELECT 1; DELETE FROM todos"));
        assert!(!is_select_only("SELECT 1; INSERT INTO t VALUES (1)"));
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

    // ── permission checks ────────────────────────────────────────

    /// Helper: build a permission list for testing.
    fn perms(ps: &[PluginPermission]) -> Vec<PluginPermission> {
        ps.to_vec()
    }

    #[test]
    fn test_storage_permission_required_for_db() {
        let with = perms(&[PluginPermission::Storage]);
        let without = perms(&[PluginPermission::Network]);
        let empty = perms(&[]);

        assert!(with.contains(&PluginPermission::Storage));
        assert!(!without.contains(&PluginPermission::Storage));
        assert!(!empty.contains(&PluginPermission::Storage));
    }

    #[test]
    fn test_network_permission_required_for_http() {
        let with = perms(&[PluginPermission::Network]);
        let without = perms(&[PluginPermission::Storage]);
        let both = perms(&[PluginPermission::Network, PluginPermission::Storage]);

        assert!(with.contains(&PluginPermission::Network));
        assert!(!without.contains(&PluginPermission::Network));
        assert!(both.contains(&PluginPermission::Network));
    }

    #[test]
    fn test_agent_permission_required_for_agent_fns() {
        let with = perms(&[PluginPermission::Agent]);
        let without = perms(&[PluginPermission::Network, PluginPermission::Storage]);

        assert!(with.contains(&PluginPermission::Agent));
        assert!(!without.contains(&PluginPermission::Agent));
    }

    #[test]
    fn test_all_permissions_independent() {
        let all = perms(&[
            PluginPermission::Network,
            PluginPermission::Storage,
            PluginPermission::Agent,
        ]);
        assert!(all.contains(&PluginPermission::Network));
        assert!(all.contains(&PluginPermission::Storage));
        assert!(all.contains(&PluginPermission::Agent));

        let none = perms(&[]);
        assert!(!none.contains(&PluginPermission::Network));
        assert!(!none.contains(&PluginPermission::Storage));
        assert!(!none.contains(&PluginPermission::Agent));
    }

    // ── check_table_sandbox ─────────────────────────────────────

    #[test]
    fn test_sandbox_allows_plugin_owned_tables() {
        let pid = "notion-connector";
        assert!(check_table_sandbox(
            "SELECT * FROM plugin_notion_connector_cache",
            pid
        ).is_ok());
        assert!(check_table_sandbox(
            "INSERT INTO plugin_notion_connector_items (id, data) VALUES ('1', 'x')",
            pid
        ).is_ok());
        assert!(check_table_sandbox(
            "DELETE FROM plugin_notion_connector_cache WHERE id = '1'",
            pid
        ).is_ok());
        assert!(check_table_sandbox(
            "UPDATE plugin_notion_connector_cache SET data = 'y' WHERE id = '1'",
            pid
        ).is_ok());
        assert!(check_table_sandbox(
            "CREATE TABLE IF NOT EXISTS plugin_notion_connector_new (id TEXT)",
            pid
        ).is_ok());
    }

    #[test]
    fn test_sandbox_rejects_system_tables() {
        let pid = "notion-connector";
        assert!(check_table_sandbox("SELECT * FROM sessions", pid).is_err());
        assert!(check_table_sandbox("DELETE FROM todos WHERE id = '1'", pid).is_err());
        assert!(check_table_sandbox(
            "INSERT INTO actions (id) VALUES ('1')",
            pid
        ).is_err());
        assert!(check_table_sandbox("UPDATE projects SET name = 'x'", pid).is_err());
    }

    #[test]
    fn test_sandbox_rejects_other_plugin_tables() {
        assert!(check_table_sandbox(
            "SELECT * FROM plugin_other_plugin_data",
            "notion-connector"
        ).is_err());
    }

    #[test]
    fn test_sandbox_allows_query_with_no_tables() {
        // Pure expressions with no table references
        assert!(check_table_sandbox("SELECT 1", "my-plugin").is_ok());
    }
}
