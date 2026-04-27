//! Coexistence guard. Runs during Phase B–D (commands in legacy AND klynt is illegal).
//! Deleted in Phase E along with `LEGACY_COMMAND_NAMES`.

use std::collections::BTreeSet;
use desktop::LEGACY_COMMAND_NAMES;
use desktop::specta_builder::KLYNT_COMMANDS;

#[test]
fn no_command_double_registered() {
    let legacy: BTreeSet<&str> = LEGACY_COMMAND_NAMES.iter().copied().collect();
    let klynt: BTreeSet<&str> = KLYNT_COMMANDS.iter().map(|c| c.name).collect();
    let overlap: Vec<&&str> = legacy.intersection(&klynt).collect();
    assert!(
        overlap.is_empty(),
        "Command in both legacy and klynt slices: {overlap:?}\n\
         Phase C migration drops names from `LEGACY_COMMAND_NAMES` as it adds them via `#[klynt_command]`.\n\
         If a name appears in both, the corresponding Phase C task forgot to remove it from main.rs."
    );
}
