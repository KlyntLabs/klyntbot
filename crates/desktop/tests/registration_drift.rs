//! Asserts the linkme slice (Tauri runtime truth) and the specta hand-list
//! (FE binding truth) contain the same set of command names.

use desktop::specta_builder::{KLYNT_COMMANDS, SPECTA_COMMAND_NAMES};
use std::collections::BTreeSet;

#[test]
fn linkme_and_specta_lists_match() {
    let linkme: BTreeSet<&str> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();
    let specta: BTreeSet<&str> = SPECTA_COMMAND_NAMES.iter().copied().collect();

    let missing: Vec<&&str> = linkme.difference(&specta).collect();
    let extra: Vec<&&str> = specta.difference(&linkme).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "Registration drift!\n  In linkme but not specta (add to collect_commands!): {missing:?}\n  In specta but not linkme (remove from collect_commands!): {extra:?}"
    );
}
