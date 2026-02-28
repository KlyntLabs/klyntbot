//! API error type with consistent JSON shape and HTTP status mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Unified API error returned by all dashboard handlers.
///
/// Serializes as `{"status": N, "message": "..."}` with matching HTTP status code.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl ApiError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: 404,
            message: msg.into(),
        }
    }

    pub fn unprocessable(msg: impl Into<String>) -> Self {
        Self {
            status: 422,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: 500,
            message: msg.into(),
        }
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: 400,
            message: msg.into(),
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: 409,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::Json(self);
        (status, body).into_response()
    }
}

impl From<storage::StorageError> for ApiError {
    fn from(e: storage::StorageError) -> Self {
        Self::from(common::KlyntbotError::from(e))
    }
}

impl From<common::KlyntbotError> for ApiError {
    fn from(e: common::KlyntbotError) -> Self {
        match &e {
            common::KlyntbotError::StorageNotFound(msg) => Self::not_found(msg.clone()),
            common::KlyntbotError::StorageConflict(msg) => Self::conflict(msg.clone()),
            // Storage errors don't leak internal details
            common::KlyntbotError::Storage(_) => Self::internal("Storage error"),
            common::KlyntbotError::Config(_) => Self {
                status: 422,
                message: e.to_string(),
            },
            _ => Self::internal(e.to_string()),
        }
    }
}
