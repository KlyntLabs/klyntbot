//! Phase 1 smoke test: programmatic mouse-move + cursor-position read.
//!
//! Gated by `KLYNT_E2E_COMPUTER_USE=1` env var so it only runs when
//! explicitly invoked. Requires the running process to have Accessibility
//! permission granted.

#![cfg(target_os = "macos")]

use platform_input::{ComputerUseAction, PlatformInput};
use platform_macos::computer_use::MacInput;

fn enabled() -> bool {
    std::env::var("KLYNT_E2E_COMPUTER_USE").as_deref() == Ok("1")
}

#[tokio::test]
async fn move_mouse_and_read_position() {
    if !enabled() {
        eprintln!("skip: set KLYNT_E2E_COMPUTER_USE=1 to run");
        return;
    }

    let input = MacInput::new().expect("MacInput::new");

    // Move to a known coordinate.
    input
        .perform_action(ComputerUseAction::MouseMove { x: 200, y: 200 })
        .await
        .expect("MouseMove");

    // Read back; allow ±2 pixel tolerance for compositor rounding.
    let pos = input
        .get_cursor_position()
        .await
        .expect("get_cursor_position");
    assert!(
        (pos.x - 200.0).abs() <= 2.0 && (pos.y - 200.0).abs() <= 2.0,
        "expected ~(200,200), got ({}, {})",
        pos.x,
        pos.y
    );
}
