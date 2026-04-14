const ADAPT_PROMPT_TEMPLATE: &str = include_str!("../prompts/adapt.md");

const EXAMPLE_BLOCK: &str = r#"metadata:
  klyntbot:
    type: orchestrator
    tools: [database]
    version: 1.0.0
    triggers: ["book", "reading"]
    bootstraps:
      databases:
        - template: reading_list.json"#;

pub fn render_prompt(
    skill_md: &str,
    supported_field_types: &[&str],
    current_databases: &[(String, String)], // (name, slug)
) -> String {
    let types_list = supported_field_types.join(", ");
    let dbs_list: String = if current_databases.is_empty() {
        "(none)".to_string()
    } else {
        current_databases
            .iter()
            .map(|(n, s)| format!("- {n} ({s})"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    ADAPT_PROMPT_TEMPLATE
        .replace("{{FIELD_TYPES}}", &types_list)
        .replace("{{CURRENT_DATABASES}}", &dbs_list)
        .replace("{{EXAMPLE_BLOCK}}", EXAMPLE_BLOCK)
        .replace("{{SKILL_MD}}", skill_md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_all_placeholders() {
        let out = render_prompt(
            "SKILL_BODY",
            &["text", "number"],
            &[("R".into(), "r".into())],
        );
        assert!(out.contains("text, number"));
        assert!(out.contains("R (r)"));
        assert!(out.contains("SKILL_BODY"));
        assert!(out.contains("example_block") || out.contains("klyntbot"));
    }
}
