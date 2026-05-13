//! Connect to the axum WS route with a bad token, expect 401.
//!
//! This test is wired up via the desktop crate's integration harness. For the
//! moment we keep it ignored — the unit-level token comparison is already
//! covered by `feature-coding-bash::attach::token::tests::tokens_eq_constant_time_basic`.

#[tokio::test]
#[ignore = "requires AppCore wiring; run with --ignored if exercising the full stack"]
async fn ws_handler_rejects_bad_token() {
    // Placeholder: actual axum/tower integration lives in desktop crate tests.
}
