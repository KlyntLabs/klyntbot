//! Klynt skill loader — extends `skill-system` with:
//! - Discovery from `~/.klyntbot/skills/` and `~/.klyntbot/project-skills/`.
//! - Path-conditional activation via `paths:` frontmatter glob.
//! - Dynamic discovery on file-touch.
//!
//! Plan 1: skeleton. Plan 5: lit up.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
