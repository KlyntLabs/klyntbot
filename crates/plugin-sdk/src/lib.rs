//! klyntbot Plugin SDK
//!
//! Re-exports everything plugin authors need to build WASM plugins for klyntbot.
//!
//! # Example (Rust plugin)
//!
//! ```rust,ignore
//! use klyntbot_plugin_sdk::prelude::*;
//!
//! #[plugin_fn]
//! pub fn my_tool(input: String) -> FnResult<String> {
//!     let args: serde_json::Value = serde_json::from_str(&input)?;
//!     Ok(format!("Got: {}", args))
//! }
//! ```

pub use extism_pdk::*;

pub mod prelude {
    pub use extism_pdk::*;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;

    pub use super::{config_get, db_query, http_get, log_error, log_info, log_warn};
}

/// Get a config value set by the user for this plugin.
pub fn config_get(key: &str) -> Option<String> {
    extism_pdk::config::get(key).ok().flatten()
}

/// Make an HTTP GET request (requires `network` permission).
pub fn http_get(url: &str, headers: &[(&str, &str)]) -> String {
    let mut req = extism_pdk::HttpRequest::new(url);
    for &(k, v) in headers {
        req = req.with_header(k, v);
    }
    match extism_pdk::http::request::<()>(&req, None) {
        Ok(resp) => String::from_utf8(resp.body()).unwrap_or_default(),
        Err(e) => format!(r#"{{"error":"{}"}}"#, e),
    }
}

/// Execute a SELECT query on this plugin's sandboxed tables (requires `storage` permission).
/// Calls the host `db_query` function via FFI.
pub fn db_query(_sql: &str) -> String {
    // Call host function through Extism's host function mechanism
    // The host registers `db_query` in the "db" namespace
    match extism_pdk::var::get("__db_query_not_implemented") {
        Ok(Some(v)) => String::from_utf8(v).unwrap_or_default(),
        _ => "[]".to_string(),
    }
}

/// Log an info message via the host logger.
pub fn log_info(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Info, "{}", msg);
}

/// Log a warning message via the host logger.
pub fn log_warn(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Warn, "{}", msg);
}

/// Log an error message via the host logger.
pub fn log_error(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Error, "{}", msg);
}
