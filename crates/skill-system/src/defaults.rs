//! Compiled default skill content — embedded at build time via `include_str!`.
//!
//! Returns all skill files in the format expected by
//! `cognitive::services::reforge::skill_files::SkillFileManager::seed_if_empty`.

use std::collections::HashMap;

/// Returns compiled default skill content as a map of
/// `skill_name → Vec<(file_path, content)>`.
///
/// The paths are relative to the skill directory root, e.g. `"SKILL.md"` or
/// `"references/cron.md"`.  Only `.md` files are included.
pub fn compiled_skill_defaults() -> HashMap<String, Vec<(&'static str, &'static str)>> {
    let mut map: HashMap<String, Vec<(&'static str, &'static str)>> = HashMap::new();

    // ── automation ───────────────────────────────────────────────────────────
    map.insert(
        "automation".to_string(),
        vec![
            (
                "SKILL.md",
                include_str!("../../../skills/automation/SKILL.md"),
            ),
            (
                "references/cron.md",
                include_str!("../../../skills/automation/references/cron.md"),
            ),
            (
                "scripts/cron_cheatsheet.md",
                include_str!("../../../skills/automation/scripts/cron_cheatsheet.md"),
            ),
        ],
    );

    // ── finance-management ───────────────────────────────────────────────────
    map.insert(
        "finance-management".to_string(),
        vec![
            (
                "SKILL.md",
                include_str!("../../../skills/finance-management/SKILL.md"),
            ),
            (
                "references/analytics-actions.md",
                include_str!("../../../skills/finance-management/references/analytics-actions.md"),
            ),
            (
                "references/budgeting.md",
                include_str!("../../../skills/finance-management/references/budgeting.md"),
            ),
            (
                "references/currency-engine.md",
                include_str!("../../../skills/finance-management/references/currency-engine.md"),
            ),
            (
                "references/financial-health.md",
                include_str!("../../../skills/finance-management/references/financial-health.md"),
            ),
            (
                "references/fire-planning.md",
                include_str!("../../../skills/finance-management/references/fire-planning.md"),
            ),
            (
                "references/portfolio-analysis.md",
                include_str!("../../../skills/finance-management/references/portfolio-analysis.md"),
            ),
            (
                "references/spending-intelligence.md",
                include_str!(
                    "../../../skills/finance-management/references/spending-intelligence.md"
                ),
            ),
            (
                "scripts/validate_amount.md",
                include_str!("../../../skills/finance-management/scripts/validate_amount.md"),
            ),
        ],
    );

    // ── learning ─────────────────────────────────────────────────────────────
    map.insert(
        "learning".to_string(),
        vec![(
            "SKILL.md",
            include_str!("../../../skills/learning/SKILL.md"),
        )],
    );

    // ── notebook ─────────────────────────────────────────────────────────────
    map.insert(
        "notebook".to_string(),
        vec![(
            "SKILL.md",
            include_str!("../../../skills/notebook/SKILL.md"),
        )],
    );

    // ── task-management ──────────────────────────────────────────────────────
    map.insert(
        "task-management".to_string(),
        vec![
            (
                "SKILL.md",
                include_str!("../../../skills/task-management/SKILL.md"),
            ),
            (
                "assets/plan_template.md",
                include_str!("../../../skills/task-management/assets/plan_template.md"),
            ),
            (
                "references/daily-planner.md",
                include_str!("../../../skills/task-management/references/daily-planner.md"),
            ),
            (
                "references/project-management.md",
                include_str!("../../../skills/task-management/references/project-management.md"),
            ),
            (
                "references/reports.md",
                include_str!("../../../skills/task-management/references/reports.md"),
            ),
            (
                "references/retrospective.md",
                include_str!("../../../skills/task-management/references/retrospective.md"),
            ),
            (
                "references/task-decompose.md",
                include_str!("../../../skills/task-management/references/task-decompose.md"),
            ),
            (
                "references/todo.md",
                include_str!("../../../skills/task-management/references/todo.md"),
            ),
            (
                "references/weekly-review.md",
                include_str!("../../../skills/task-management/references/weekly-review.md"),
            ),
        ],
    );

    map
}
