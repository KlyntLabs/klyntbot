//! Klynt hook engine — 13-event Claude-Code-compatible schema.
//!
//! Reads `~/.klyntbot/hooks.toml`, dispatches subprocess hooks at the 13
//! event boundaries listed in spec §7.
//!
//! Plan 1: skeleton. Plan 4: vendor + light up.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
