use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct HelpEntry {
    pub command: String,
    pub description: String,
    pub category: String,
    pub arg_hint: Option<String>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_help(&self, _command: Option<String>) -> Result<Vec<HelpEntry>> {
        // Mirror the TS slash registry exactly.
        Ok(vec![
            HelpEntry {
                command: "/plan".into(),
                description: "Enter plan mode".into(),
                category: "mode".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/yolo".into(),
                description: "Bypass approvals".into(),
                category: "mode".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/power on".into(),
                description: "Enable power tools".into(),
                category: "mode".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/power off".into(),
                description: "Disable power tools".into(),
                category: "mode".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/recall".into(),
                description: "Force recall pass".into(),
                category: "recall".into(),
                arg_hint: Some("<query>".into()),
            },
            HelpEntry {
                command: "/skills list".into(),
                description: "List installed skills".into(),
                category: "skills".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/skills info".into(),
                description: "Show skill frontmatter".into(),
                category: "skills".into(),
                arg_hint: Some("<name>".into()),
            },
            HelpEntry {
                command: "/skills install".into(),
                description: "Install a skill".into(),
                category: "skills".into(),
                arg_hint: Some("<source>".into()),
            },
            HelpEntry {
                command: "/skills update".into(),
                description: "Re-fetch skill".into(),
                category: "skills".into(),
                arg_hint: Some("<name>".into()),
            },
            HelpEntry {
                command: "/skills uninstall".into(),
                description: "Remove skill".into(),
                category: "skills".into(),
                arg_hint: Some("<name>".into()),
            },
            HelpEntry {
                command: "/skills toggle".into(),
                description: "Enable/disable skill".into(),
                category: "skills".into(),
                arg_hint: Some("<name> --on|--off".into()),
            },
            HelpEntry {
                command: "/skills validate".into(),
                description: "Check SKILL.md".into(),
                category: "skills".into(),
                arg_hint: Some("<name>".into()),
            },
            HelpEntry {
                command: "/skills reload".into(),
                description: "Re-walk discovery".into(),
                category: "skills".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/status".into(),
                description: "Show mode/profile/sandbox/cost".into(),
                category: "status".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/doctor".into(),
                description: "Run diagnostic".into(),
                category: "status".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/sessions star".into(),
                description: "Star current thread".into(),
                category: "sessions".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/sessions unstar".into(),
                description: "Unstar".into(),
                category: "sessions".into(),
                arg_hint: None,
            },
            HelpEntry {
                command: "/resume".into(),
                description: "Switch to matching thread".into(),
                category: "sessions".into(),
                arg_hint: Some("<prefix>".into()),
            },
            HelpEntry {
                command: "/help".into(),
                description: "List commands".into(),
                category: "help".into(),
                arg_hint: Some("[command]".into()),
            },
        ])
    }
}
