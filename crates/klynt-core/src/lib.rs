//! Klynt core — coding-tool registry + glue between execpolicy, sandbox,
//! hooks, and the agent loop.
//!
//! Plan 1: skeleton.
//! Plan 2: bash tool + macOS Seatbelt + ApprovalCard wiring.
//! Plan 3: read/glob/grep/edit/write/apply_patch/web_fetch/notebook_edit.
//! Plan 4: hook engine integration; Layer 2 wiring.
//! Plan 5: skill-loader integration; recall_* tool registration.

pub mod tools {
    //! Coding tools (bash, read, glob, grep, edit, write, apply_patch, …).
    //! Plans 2-3 add implementations.
}

pub mod slash {
    //! Slash-command direct-dispatch handlers (skills, status, doctor, …).
    //! Plan 6 adds implementations.
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
