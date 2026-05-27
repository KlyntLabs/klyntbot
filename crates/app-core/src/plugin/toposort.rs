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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PluginContext;
    use async_trait::async_trait;

    /// Minimal plugin used only to exercise `resolve_order`, which never calls
    /// `init` — it reads `name()` and `dependencies()` only.
    struct MockPlugin {
        name: &'static str,
        deps: Vec<&'static str>,
    }

    impl MockPlugin {
        fn new(name: &'static str, deps: &[&'static str]) -> Box<dyn AppCorePlugin> {
            Box::new(MockPlugin {
                name,
                deps: deps.to_vec(),
            })
        }
    }

    #[async_trait]
    impl AppCorePlugin for MockPlugin {
        fn name(&self) -> &str {
            self.name
        }
        fn dependencies(&self) -> &[&str] {
            &self.deps
        }
        async fn init(&self, _ctx: &mut PluginContext) -> common::Result<()> {
            Ok(())
        }
    }

    fn names(plugins: &[Box<dyn AppCorePlugin>]) -> Vec<&str> {
        plugins.iter().map(|p| p.name()).collect()
    }

    /// `unwrap_err` requires the Ok type to be `Debug`; `Vec<Box<dyn AppCorePlugin>>`
    /// is not, so extract the error manually.
    fn expect_err(r: common::Result<Vec<Box<dyn AppCorePlugin>>>) -> common::KlyntbotError {
        match r {
            Ok(_) => panic!("expected resolve_order to fail"),
            Err(e) => e,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(resolve_order(vec![]).unwrap().is_empty());
    }

    #[test]
    fn no_dependencies_preserves_all_plugins() {
        let out =
            resolve_order(vec![MockPlugin::new("a", &[]), MockPlugin::new("b", &[])]).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn dependency_is_ordered_before_dependent() {
        // "b" depends on "a" but is registered first; the sort must reorder.
        let out = resolve_order(vec![
            MockPlugin::new("b", &["a"]),
            MockPlugin::new("a", &[]),
        ])
        .unwrap();
        let out = names(&out);
        let pos_a = out.iter().position(|n| *n == "a").unwrap();
        let pos_b = out.iter().position(|n| *n == "b").unwrap();
        assert!(pos_a < pos_b, "expected 'a' before 'b', got {out:?}");
    }

    #[test]
    fn transitive_chain_is_fully_ordered() {
        // c -> b -> a, registered in reverse; result must be a, b, c.
        let out = resolve_order(vec![
            MockPlugin::new("c", &["b"]),
            MockPlugin::new("b", &["a"]),
            MockPlugin::new("a", &[]),
        ])
        .unwrap();
        assert_eq!(names(&out), vec!["a", "b", "c"]);
    }

    #[test]
    fn missing_dependency_is_an_error() {
        let err = expect_err(resolve_order(vec![MockPlugin::new("a", &["ghost"])]));
        let msg = err.to_string();
        assert!(
            msg.contains("ghost"),
            "expected dep name in error, got: {msg}"
        );
        assert!(msg.contains("not registered"), "got: {msg}");
    }

    #[test]
    fn direct_cycle_is_detected() {
        let err = expect_err(resolve_order(vec![
            MockPlugin::new("a", &["b"]),
            MockPlugin::new("b", &["a"]),
        ]));
        assert!(err.to_string().contains("cycle"), "got: {err}");
    }

    #[test]
    fn self_cycle_is_detected() {
        let err = expect_err(resolve_order(vec![MockPlugin::new("a", &["a"])]));
        assert!(err.to_string().contains("cycle"), "got: {err}");
    }
}
