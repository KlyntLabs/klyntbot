//! Static registry of all feature packs.

use config::PackTier;

/// A feature pack bundles config settings and skills.
#[derive(Debug, Clone)]
pub struct Pack {
    /// Unique identifier (e.g., "productivity").
    pub id: &'static str,
    /// Display name (e.g., "Productivity").
    pub name: &'static str,
    /// Short description for the wizard UI.
    pub description: &'static str,
    /// Selection tier (Core/Recommended/Optional).
    pub tier: PackTier,
    /// Skill names included in this pack.
    pub skills: &'static [&'static str],
}

/// Static pack registry.
pub struct PackRegistry;

impl PackRegistry {
    /// All defined packs, ordered by tier then name.
    pub fn all() -> Vec<&'static Pack> {
        PACKS.iter().collect()
    }

    /// Packs matching a given tier.
    pub fn by_tier(tier: PackTier) -> Vec<&'static Pack> {
        PACKS.iter().filter(|p| p.tier == tier).collect()
    }

    /// Look up a pack by ID.
    pub fn get(id: &str) -> Option<&'static Pack> {
        PACKS.iter().find(|p| p.id == id)
    }

    /// Collect all skill names from a set of enabled pack IDs.
    pub fn skills_for_packs(enabled: &[String]) -> Vec<String> {
        let mut skills = Vec::new();
        for pack in PACKS.iter() {
            if enabled.iter().any(|e| e == pack.id) {
                for skill in pack.skills {
                    let s = skill.to_string();
                    if !skills.contains(&s) {
                        skills.push(s);
                    }
                }
            }
        }
        skills
    }

    /// Default selection: Core + Recommended packs.
    pub fn default_selection() -> Vec<String> {
        PACKS
            .iter()
            .filter(|p| p.tier == PackTier::Core || p.tier == PackTier::Recommended)
            .map(|p| p.id.to_string())
            .collect()
    }
}

static PACKS: &[Pack] = &[
    Pack {
        id: "task-management",
        name: "Task Management",
        description: "Tasks, focus mode, enrichment, semantic search",
        tier: PackTier::Core,
        skills: &["todo"],
    },
    Pack {
        id: "productivity",
        name: "Productivity",
        description: "Daily planning, cron, summarize, activity tracking",
        tier: PackTier::Recommended,
        skills: &[
            "daily-planning",
            "cron",
            "summarize",
            "productivity-tracking",
        ],
    },
    Pack {
        id: "ai-intelligence",
        name: "AI Intelligence",
        description: "Conversation memory, learning system, embeddings",
        tier: PackTier::Recommended,
        skills: &[],
    },
    Pack {
        id: "browser",
        name: "Browser Automation",
        description: "Real-world task execution: booking, shopping, account management",
        tier: PackTier::Optional,
        skills: &["browser"],
    },
    Pack {
        id: "finance",
        name: "Finance",
        description: "Budget tracking, expenses, investment projection",
        tier: PackTier::Optional,
        skills: &["finance"],
    },
    Pack {
        id: "skill-creator",
        name: "Skill Creator",
        description: "Create custom skills",
        tier: PackTier::Optional,
        skills: &["skill-creator"],
    },
    Pack {
        id: "weather",
        name: "Weather",
        description: "Weather queries and forecasts",
        tier: PackTier::Optional,
        skills: &["weather"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_packs_returns_7() {
        let packs = PackRegistry::all();
        assert_eq!(packs.len(), 7);
    }

    #[test]
    fn test_core_packs_not_empty() {
        let core = PackRegistry::by_tier(PackTier::Core);
        assert!(!core.is_empty());
        assert!(core.iter().all(|p| p.tier == PackTier::Core));
    }

    #[test]
    fn test_recommended_packs() {
        let rec = PackRegistry::by_tier(PackTier::Recommended);
        assert_eq!(rec.len(), 2);
    }

    #[test]
    fn test_optional_packs() {
        let opt = PackRegistry::by_tier(PackTier::Optional);
        assert_eq!(opt.len(), 4);
    }

    #[test]
    fn test_get_by_id() {
        let pack = PackRegistry::get("productivity");
        assert!(pack.is_some());
        assert_eq!(pack.unwrap().name, "Productivity");
    }

    #[test]
    fn test_get_unknown_returns_none() {
        assert!(PackRegistry::get("nonexistent").is_none());
    }

    #[test]
    fn test_task_management_skills() {
        let pack = PackRegistry::get("task-management").unwrap();
        assert!(pack.skills.contains(&"todo"));
    }

    #[test]
    fn test_skills_for_packs() {
        let enabled = vec!["task-management".to_string(), "productivity".to_string()];
        let skills = PackRegistry::skills_for_packs(&enabled);
        assert!(skills.contains(&"todo".to_string()));
        assert!(skills.contains(&"daily-planning".to_string()));
        assert!(!skills.contains(&"weather".to_string()));
    }

    #[test]
    fn test_default_selection() {
        let selection = PackRegistry::default_selection();
        assert!(selection.contains(&"task-management".to_string()));
        assert!(selection.contains(&"productivity".to_string()));
        assert!(!selection.contains(&"finance".to_string()));
    }

    #[test]
    fn test_all_pack_ids_unique() {
        let packs = PackRegistry::all();
        let mut ids: Vec<&str> = packs.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), packs.len());
    }
}
