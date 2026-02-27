//! Embedded frontend assets and SPA fallback handler.
//!
//! In release builds, `include_dir!` bakes `frontend/dist/` into the binary.
//! During development (when `dist/` is absent), the handler returns a stub
//! so the backend can be tested independently of the frontend build step.

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::Response;
use include_dir::{include_dir, Dir};

/// Embedded frontend distribution directory.
///
/// If `frontend/dist/` doesn't exist at compile time, `include_dir!` embeds
/// an empty directory — the SPA fallback returns a development stub instead
/// of panicking.
static FRONTEND_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

/// Serve a static asset from the embedded dist directory, or fall back to
/// `index.html` for SPA client-side routing (any non-asset path).
pub async fn spa_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact file match first
    if let Some(file) = FRONTEND_DIR.get_file(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .body(Body::from(file.contents()))
            .unwrap();
    }

    // Fall back to index.html for SPA routing
    serve_index()
}

fn serve_index() -> Response {
    if let Some(index) = FRONTEND_DIR.get_file("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(index.contents()))
            .unwrap();
    }

    // Development stub: frontend not built yet
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(
            "<!doctype html><html><body>\
             <h1>Klyntbot Dashboard</h1>\
             <p>Frontend not built. Run <code>npm run build</code> in \
             <code>crates/dashboard/frontend/</code>.</p>\
             </body></html>",
        ))
        .unwrap()
}
