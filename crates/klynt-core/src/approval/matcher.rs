use globset::{Glob, GlobSet, GlobSetBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MatcherError {
    #[error("malformed rule {0:?}: must be Tool(glob)")]
    BadShape(String),
    #[error("invalid glob: {0}")]
    Glob(#[from] globset::Error),
}

pub struct CompiledRules {
    sets: std::collections::HashMap<String, (GlobSet, Vec<String>)>,
}

impl CompiledRules {
    pub fn compile(patterns: &[String]) -> Result<Self, MatcherError> {
        let mut buckets: std::collections::HashMap<String, GlobSetBuilder> = Default::default();
        let mut raws: std::collections::HashMap<String, Vec<String>> = Default::default();
        for p in patterns {
            let (tool, glob) = parse_rule(p)?;
            buckets
                .entry(tool.clone())
                .or_insert_with(GlobSetBuilder::new)
                .add(Glob::new(&glob)?);
            raws.entry(tool).or_default().push(p.clone());
        }
        let sets = buckets
            .into_iter()
            .map(|(t, b)| {
                let r = raws.remove(&t).unwrap_or_default();
                Ok::<_, MatcherError>((t, (b.build()?, r)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { sets })
    }
    pub fn find_match(&self, tool: &str, payload: &str) -> Option<String> {
        let (set, raws) = self.sets.get(&normalize_tool(tool))?;
        let m: Vec<usize> = set.matches(payload);
        m.first().map(|&i| raws[i].clone())
    }
}

/// Normalize a tool name for matcher lookup so that `ApplyPatch`, `apply_patch`,
/// and `applypatch` all hash to the same bucket.
fn normalize_tool(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '_' | '-'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn parse_rule(rule: &str) -> Result<(String, String), MatcherError> {
    let open = rule
        .find('(')
        .ok_or_else(|| MatcherError::BadShape(rule.into()))?;
    if !rule.ends_with(')') {
        return Err(MatcherError::BadShape(rule.into()));
    }
    Ok((
        normalize_tool(rule[..open].trim()),
        rule[open + 1..rule.len() - 1].to_string(),
    ))
}
