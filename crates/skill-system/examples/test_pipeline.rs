//! Quick smoke test for the skill system pipeline.
//! Run: cargo run -p skill-system --example test_pipeline

use skill_system::discovery::{
    builtin_reference_map, builtin_skills_info, SkillSource, BUILTIN_SKILLS,
};
use skill_system::router::SkillRouter;
use skill_system::types::SkillCatalog;

fn main() {
    println!("=== Skill System Pipeline Test ===\n");

    // 1. Discover built-in skills
    let source = SkillSource::BuiltIn(
        BUILTIN_SKILLS
            .iter()
            .map(|(n, c)| (n.to_string(), c.to_string()))
            .collect(),
    );
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();

    println!("1. Discovery:");
    println!("   Orchestrators: {}", catalog.orchestrators().len());
    println!("   Regular skills: {}", catalog.regular_skills().len());
    for pkg in catalog.orchestrators() {
        println!(
            "   - {} (type: orchestrator, tools: {:?})",
            pkg.name,
            pkg.allowed_tool_names().map(|s| s.len())
        );
    }
    println!();

    // 2. Router tests
    let router = SkillRouter::new(&catalog);
    let test_messages = [
        ("create a task for my project planning", "task-management"),
        ("check my budget spending", "finance-management"),
        ("hello there!", "general"),
        ("set a reminder for tomorrow", "automation"),
        ("send a message to the team", "communication"),
        ("what is the weather today?", "general"),
        ("schedule a recurring job every hour", "automation"),
        ("list my todos and plan for the week", "task-management"),
    ];

    println!("2. Router (keyword-only):");
    let mut pass = 0;
    let mut fail = 0;
    for (msg, expected) in &test_messages {
        let selected = router.select_orchestrator(msg, &catalog, None);
        let status = if selected.name == *expected {
            pass += 1;
            "PASS"
        } else {
            fail += 1;
            "FAIL"
        };
        println!(
            "   [{status}] \"{msg}\" → {} (expected: {expected})",
            selected.name
        );
    }
    println!("   Results: {pass} passed, {fail} failed\n");

    // 3. Reference files
    let refs = builtin_reference_map();
    println!("3. Reference files: {} loaded", refs.len());
    for key in refs.keys() {
        println!("   - {key}");
    }
    println!();

    // 4. Catalog prompt (Tier 1)
    let prompt = catalog.catalog_prompt();
    println!("4. Catalog prompt ({} chars):", prompt.len());
    println!("{prompt}");
    println!();

    // 5. UI info
    let info = builtin_skills_info();
    println!("5. UI skill info:");
    for s in &info {
        println!("   {} ({} references)", s.name, s.references.len());
    }
    println!();

    if fail > 0 {
        println!("RESULT: {} routing failures — review descriptions", fail);
        std::process::exit(1);
    } else {
        println!("RESULT: All tests passed!");
    }
}
