use crate::types::*;

struct CommandDef {
    title: &'static str,
    subtitle: &'static str,
    action: SystemAction,
    keywords: &'static [&'static str],
    arguments: &'static [(&'static str, &'static str, bool)], // (name, placeholder, required)
    no_view: bool,
}

const COMMANDS: &[CommandDef] = &[
    CommandDef {
        title: "Lock Screen",
        subtitle: "Lock the display",
        action: SystemAction::LockScreen,
        keywords: &["lock", "screen", "security"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Sleep",
        subtitle: "Put the Mac to sleep",
        action: SystemAction::Sleep,
        keywords: &["sleep", "suspend"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Restart",
        subtitle: "Restart the Mac",
        action: SystemAction::Restart,
        keywords: &["restart", "reboot"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Shutdown",
        subtitle: "Shut down the Mac",
        action: SystemAction::Shutdown,
        keywords: &["shutdown", "power off", "turn off"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Empty Trash",
        subtitle: "Empty the Trash",
        action: SystemAction::EmptyTrash,
        keywords: &["trash", "empty", "clean"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Toggle Dark Mode",
        subtitle: "Switch appearance",
        action: SystemAction::ToggleDarkMode,
        keywords: &["dark", "light", "mode", "appearance", "theme"],
        arguments: &[],
        no_view: false,
    },
    CommandDef {
        title: "Toggle Do Not Disturb",
        subtitle: "Toggle Focus mode",
        action: SystemAction::ToggleDoNotDisturb,
        keywords: &["disturb", "dnd", "focus", "notifications", "quiet"],
        arguments: &[("duration", "30m, 2h, until tomorrow", false)],
        no_view: true,
    },
    CommandDef {
        title: "Eject All",
        subtitle: "Eject all external drives",
        action: SystemAction::EjectAll,
        keywords: &["eject", "drives", "unmount"],
        arguments: &[],
        no_view: false,
    },
];

fn arg_specs(defs: &'static [(&'static str, &'static str, bool)]) -> Vec<ArgSpec> {
    defs.iter()
        .map(|(name, placeholder, required)| ArgSpec {
            name: (*name).to_string(),
            placeholder: (*placeholder).to_string(),
            kind: ArgKind::Text,
            required: *required,
        })
        .collect()
}

pub struct SystemCommands;

impl SystemCommands {
    pub fn search(query: &str) -> Vec<LauncherItem> {
        if query.is_empty() {
            return COMMANDS
                .iter()
                .map(|cmd| LauncherItem {
                    id: format!("system:{:?}", cmd.action),
                    title: cmd.title.to_string(),
                    subtitle: Some(cmd.subtitle.to_string()),
                    icon: Some("terminal".to_string()),
                    kind: LauncherItemKind::SystemCommand {
                        action: cmd.action.clone(),
                    },
                    score: 0.5,
                    no_view: cmd.no_view,
                    arguments: arg_specs(cmd.arguments),
                })
                .collect();
        }

        // Score on title first via fuzzy_match, then boost with keyword matches
        use nucleo_matcher::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher, Utf32Str,
        };

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut scored: Vec<(u32, &CommandDef)> = COMMANDS
            .iter()
            .filter_map(|cmd| {
                let mut buf = Vec::new();
                let title_score = pattern.score(Utf32Str::new(cmd.title, &mut buf), &mut matcher);
                let keyword_score = cmd
                    .keywords
                    .iter()
                    .filter_map(|k| {
                        let mut buf2 = Vec::new();
                        pattern.score(Utf32Str::new(k, &mut buf2), &mut matcher)
                    })
                    .max();
                let best = match (title_score, keyword_score) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                best.map(|s| (s, cmd))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        scored
            .into_iter()
            .map(|(score, cmd)| LauncherItem {
                id: format!("system:{:?}", cmd.action),
                title: cmd.title.to_string(),
                subtitle: Some(cmd.subtitle.to_string()),
                icon: Some("terminal".to_string()),
                kind: LauncherItemKind::SystemCommand {
                    action: cmd.action.clone(),
                },
                score: (score as f64) / 1000.0 * 1.0,
                no_view: cmd.no_view,
                arguments: arg_specs(cmd.arguments),
            })
            .collect()
    }

    #[cfg(target_os = "macos")]
    pub async fn execute(action: &SystemAction) -> common::Result<()> {
        use std::process::Command;
        match action {
            SystemAction::LockScreen => {
                Command::new("pmset").args(["displaysleepnow"]).spawn()?;
            }
            SystemAction::Sleep => {
                Command::new("pmset").args(["sleepnow"]).spawn()?;
            }
            SystemAction::Restart => {
                Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to restart"])
                    .spawn()?;
            }
            SystemAction::Shutdown => {
                Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to shut down"])
                    .spawn()?;
            }
            SystemAction::EmptyTrash => {
                Command::new("osascript")
                    .args(["-e", "tell application \"Finder\" to empty trash"])
                    .spawn()?;
            }
            SystemAction::ToggleDarkMode => {
                Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to tell appearance preferences to set dark mode to not dark mode"])
                    .spawn()?;
            }
            SystemAction::ToggleDoNotDisturb => {
                Command::new("shortcuts")
                    .args(["run", "Toggle Do Not Disturb"])
                    .spawn()?;
            }
            SystemAction::EjectAll => {
                Command::new("osascript")
                    .args([
                        "-e",
                        "tell application \"Finder\" to eject (every disk whose ejectable is true)",
                    ])
                    .spawn()?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub async fn execute(_action: &SystemAction) -> common::Result<()> {
        Err(common::KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "System commands only supported on macOS",
        )))
    }
}

#[async_trait::async_trait]
impl super::SearchSource for SystemCommands {
    fn name(&self) -> &'static str {
        "system_commands"
    }

    fn prefix(&self) -> Option<&'static str> {
        Some(">")
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<crate::types::LauncherItem> {
        let mut results = Self::search(query);
        results.truncate(limit);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_exact() {
        let results = SystemCommands::search("lock");
        assert_eq!(results.len(), 1);
        assert!(matches!(
            results[0].kind,
            LauncherItemKind::SystemCommand {
                action: SystemAction::LockScreen
            }
        ));
    }

    #[test]
    fn test_search_fuzzy() {
        let results = SystemCommands::search("dark");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_empty_returns_all() {
        let results = SystemCommands::search("");
        assert_eq!(results.len(), 8);
    }

    #[test]
    fn dnd_command_has_duration_arg() {
        let items = SystemCommands::search("dnd");
        let dnd = items
            .iter()
            .find(|i| i.title.to_lowercase().contains("do not disturb"))
            .expect("dnd item present");
        assert_eq!(dnd.arguments.len(), 1);
        assert_eq!(dnd.arguments[0].name, "duration");
        assert_eq!(dnd.arguments[0].placeholder, "30m, 2h, until tomorrow");
        assert!(!dnd.arguments[0].required);
        assert!(dnd.no_view);
    }
}
