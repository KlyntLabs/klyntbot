//! Klynt sandbox — OS-level sandboxing for tool execution.
//!
//! macOS: Seatbelt via `sandbox-exec` + generated .sbpl policies.
//! Linux: Landlock + bwrap via klynt-sandbox-helper child binary.
//!
//! Plan 1: skeleton. Plan 2: macOS Seatbelt lit up. Plan 3: Linux.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
