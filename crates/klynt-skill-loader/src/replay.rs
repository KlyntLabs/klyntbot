use crate::activator::SkillActivator;
use crate::index::DiscoveryRoots;
use common::Result;
use std::path::Path;

/// Re-activate path-conditional skills by replaying file-touch history
/// in deterministic order. Returns all skill names ever activated by
/// this replay (sorted, deduplicated).
pub fn replay_session_history(
    activator: &mut SkillActivator,
    history_paths: &[std::path::PathBuf],
    roots: &DiscoveryRoots,
) -> Result<Vec<String>> {
    let mut sorted: Vec<&Path> = history_paths.iter().map(|p| p.as_path()).collect();
    sorted.sort();
    sorted.dedup();
    let mut all = std::collections::BTreeSet::new();
    for p in sorted {
        for name in activator.touch_path_with_discovery(p, roots)? {
            all.insert(name);
        }
    }
    Ok(all.into_iter().collect())
}
