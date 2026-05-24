use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use super::AppCorePlugin;

/// Topologically sort plugins by their declared dependencies (Kahn's algorithm).
///
/// Returns an error if a dependency is missing or if a cycle exists.
pub fn resolve_order(
    plugins: Vec<Box<dyn AppCorePlugin>>,
) -> common::Result<Vec<Box<dyn AppCorePlugin>>> {
    let n = plugins.len();
    if n == 0 {
        return Ok(plugins);
    }

    // name -> index
    let name_to_idx: HashMap<&str, usize> = plugins
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name(), i))
        .collect();

    // in_degree[i] = number of dependencies plugin i has
    let mut in_degree = vec![0usize; n];
    // adjacency: for each plugin, list plugins that depend on it
    let mut dependents: Vec<Vec<usize>> = vec![vec![]; n];

    for (i, plugin) in plugins.iter().enumerate() {
        for dep in plugin.dependencies() {
            let j = *name_to_idx.get(dep).ok_or_else(|| {
                common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                    "plugin '{}' declares dependency '{}' which is not registered",
                    plugin.name(),
                    dep
                )))
            })?;
            dependents[j].push(i);
            in_degree[i] += 1;
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .enumerate()
        .filter_map(|(i, &d)| if d == 0 { Some(i) } else { None })
        .collect();
    let mut sorted = Vec::with_capacity(n);

    while let Some(i) = queue.pop_front() {
        sorted.push(i);
        for &dep in &dependents[i] {
            in_degree[dep] -= 1;
            if in_degree[dep] == 0 {
                queue.push_back(dep);
            }
        }
    }

    if sorted.len() != n {
        let unresolved: Vec<&str> = (0..n)
            .filter(|i| !sorted.contains(i))
            .map(|i| plugins[i].name())
            .collect();
        return Err(common::KlyntbotError::Config(common::ConfigError::Invalid(
            format!(
                "plugin dependency cycle detected (unresolved: {:?})",
                unresolved
            ),
        )));
    }

    // Reorder plugins according to sorted indices
    let mut plugins_opt: Vec<Option<Box<dyn AppCorePlugin>>> =
        plugins.into_iter().map(Some).collect();
    let mut result = Vec::with_capacity(n);
    for i in sorted {
        result.push(plugins_opt[i].take().unwrap());
    }
    Ok(result)
}
