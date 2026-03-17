/// Build the scenario reasoning planning prompt.
///
/// Injected when the intent analyzer detects hypothetical framing
/// (e.g., "what if I deprioritize Project X?"). The prompt guides the
/// ReAct engine through a 5-step reasoning process using graph neighborhoods.
pub fn build_scenario_prompt(
    user_message: &str,
    tools: &[serde_json::Value],
    max_graph_depth: u32,
) -> String {
    use common::helpers::tool_def_name;

    let tool_names: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();

    format!(
        "The user is exploring a scenario. Structure your response using these steps:\n\
         \n\
         1. **Identify the change variable and current baseline** — use tools to get real data \
            about the current state of what the user wants to change.\n\
         2. **Trace first-order effects** — what changes directly as a result?\n\
         3. **Trace second-order effects** — use knowledge graph neighborhoods (max depth {max_graph_depth}) \
            to find connected entities. For each connected entity, query the relevant tool.\n\
         4. **Present best/worst/likely outcomes** with specific numbers where possible.\n\
         5. **Synthesize a recommendation** — what should the user do?\n\
         \n\
         User scenario: {user_message}\n\
         Available tools: [{}]\n\
         \n\
         Start by identifying the change variable and querying for baseline data.",
        tool_names.join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_prompt_contains_key_elements() {
        let tools = vec![serde_json::json!({
            "function": { "name": "tasks" }
        })];
        let prompt = build_scenario_prompt("what if I delay the launch", &tools, 2);

        assert!(prompt.contains("what if I delay the launch"));
        assert!(prompt.contains("tasks"));
        assert!(prompt.contains("max depth 2"));
        assert!(prompt.contains("first-order effects"));
        assert!(prompt.contains("second-order effects"));
        assert!(prompt.contains("recommendation"));
    }

    #[test]
    fn test_scenario_prompt_respects_depth() {
        let tools = vec![];
        let prompt = build_scenario_prompt("test", &tools, 3);
        assert!(prompt.contains("max depth 3"));
    }
}
