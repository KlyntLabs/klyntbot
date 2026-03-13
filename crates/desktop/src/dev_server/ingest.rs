use axum::extract::State;
use axum::Json;

use super::{err, ok, ApiResult, DevState};

// ── Ingestion API handlers ────────────────────────────────────────────

pub(super) async fn ingest_handler(
    State(state): State<DevState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<desktop_shared::commands::IngestRequest>,
) -> ApiResult {
    let token = extract_bearer_token(&headers);
    match state.core.ingest_event(&token, req).await {
        Ok(resp) => ok(resp),
        Err(e) => err(e),
    }
}

pub(super) async fn ingest_batch_handler(
    State(state): State<DevState>,
    headers: axum::http::HeaderMap,
    Json(reqs): Json<Vec<desktop_shared::commands::IngestRequest>>,
) -> ApiResult {
    let token = extract_bearer_token(&headers);
    match state.core.ingest_batch(&token, reqs).await {
        Ok(resp) => ok(resp),
        Err(e) => err(e),
    }
}

fn extract_bearer_token(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_string()
}
