//! Klynt protocol types — adapted from `codex-rs/protocol/`.
//!
//! This crate is a foundation skeleton in Plan 1; Plan 2 vendors the
//! Codex protocol types and renames `Codex*` → `Klynt*` per
//! `scripts/adapt_codex_vendor.sh`.
//!
//! See spec: docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md §3.

// Public API surface — empty until Plan 2 vendoring lands.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {
        // Sentinel: this test exists so the crate has at least one
        // unit test from day one. Plan 2 replaces this when vendored
        // tests land.
    }
}
