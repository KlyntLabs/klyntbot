//! Unified browser registry and browser-related utilities.

use std::path::PathBuf;

/// Definition of a known browser.
#[derive(Debug, Clone)]
pub struct BrowserDef {
    /// Human-readable app name (as reported by macOS).
    pub name: &'static str,
    /// Bundle ID prefix for matching (empty string if none).
    pub bundle_prefix: &'static str,
    /// Chromium user-data profile subdirectory under ~/Library/Application Support/
    /// (None for non-Chromium browsers).
    pub profile_dir: Option<&'static str>,
    /// Suffix appended to window titles by this browser.
    pub title_suffix: &'static str,
}

/// Registry of known browsers, unified from feature-launcher and feature-productivity.
pub const BROWSERS: &[BrowserDef] = &[
    BrowserDef {
        name: "Google Chrome",
        bundle_prefix: "com.google.chrome",
        profile_dir: Some("Google/Chrome/Default"),
        title_suffix: " - Google Chrome",
    },
    BrowserDef {
        name: "Arc",
        bundle_prefix: "company.thebrowser.browser",
        profile_dir: Some("Arc/User Data/Default"),
        title_suffix: " - Arc",
    },
    BrowserDef {
        name: "Brave Browser",
        bundle_prefix: "com.brave.browser",
        profile_dir: Some("BraveSoftware/Brave-Browser/Default"),
        title_suffix: " - Brave",
    },
    BrowserDef {
        name: "Microsoft Edge",
        bundle_prefix: "com.microsoft.edgemac",
        profile_dir: Some("Microsoft Edge/Default"),
        title_suffix: " - Microsoft Edge",
    },
    BrowserDef {
        name: "Safari",
        bundle_prefix: "com.apple.safari",
        profile_dir: None,
        title_suffix: " - Safari",
    },
    BrowserDef {
        name: "Firefox",
        bundle_prefix: "org.mozilla.firefox",
        profile_dir: None,
        title_suffix: " - Mozilla Firefox",
    },
    BrowserDef {
        name: "Vivaldi",
        bundle_prefix: "com.vivaldi.vivaldi",
        profile_dir: None,
        title_suffix: " - Vivaldi",
    },
    BrowserDef {
        name: "Opera",
        bundle_prefix: "com.operasoftware.opera",
        profile_dir: None,
        title_suffix: " - Opera",
    },
    BrowserDef {
        name: "Chromium",
        bundle_prefix: "",
        profile_dir: None,
        title_suffix: " - Chromium",
    },
    BrowserDef {
        name: "Orion",
        bundle_prefix: "",
        profile_dir: None,
        title_suffix: " - Orion",
    },
    BrowserDef {
        name: "Zen Browser",
        bundle_prefix: "",
        profile_dir: None,
        title_suffix: " - Zen",
    },
];

/// Check whether an app is a known browser by name or bundle ID.
pub fn is_browser(app_name: &str, bundle_id: Option<&str>) -> bool {
    let name_lower = app_name.to_lowercase();
    for b in BROWSERS {
        if name_lower == b.name.to_lowercase() {
            return true;
        }
    }
    if let Some(bid) = bundle_id {
        let bid_lower = bid.to_lowercase();
        for b in BROWSERS {
            if !b.bundle_prefix.is_empty() && bid_lower.starts_with(b.bundle_prefix) {
                return true;
            }
        }
    }
    false
}

/// Resolve a Chromium-based browser's profile directory.
///
/// The `browser_name` should be a short key: `"chrome"`, `"arc"`, `"brave"`, or `"edge"`.
/// Returns `~/Library/Application Support/<profile_dir>` or `None` for unknown/non-Chromium browsers.
pub fn chromium_profile_dir(browser_name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let app_support = PathBuf::from(&home).join("Library/Application Support");
    match browser_name {
        "chrome" => Some(app_support.join("Google/Chrome/Default")),
        "arc" => Some(app_support.join("Arc/User Data/Default")),
        "brave" => Some(app_support.join("BraveSoftware/Brave-Browser/Default")),
        "edge" => Some(app_support.join("Microsoft Edge/Default")),
        _ => None,
    }
}

/// Extract a human-readable site name from a browser window title by stripping
/// the browser suffix.
///
/// For example: `"GitHub - Google Chrome"` -> `Some("GitHub")`.
/// Returns `None` if the title doesn't end with any known browser suffix.
pub fn extract_site_name(window_title: &str) -> Option<String> {
    let title_lower = window_title.to_lowercase();
    for b in BROWSERS {
        let suffix_lower = b.title_suffix.to_lowercase();
        if let Some(pos) = title_lower.rfind(&suffix_lower) {
            let name = window_title[..pos].trim();
            if name.is_empty() {
                return None;
            }
            return Some(name.to_string());
        }
    }
    None
}

/// Get the URL of the active browser tab via AppleScript.
///
/// Works for Chrome-family browsers (Chrome, Brave, Vivaldi, Edge, Opera, Arc)
/// and Safari. Returns `None` for non-browser apps or if the script fails.
#[cfg(target_os = "macos")]
pub fn get_browser_url(app_name: &str, bundle_id: Option<&str>) -> Option<String> {
    if !is_browser(app_name, bundle_id) {
        return None;
    }

    let name_lower = app_name.to_lowercase();

    let script = if name_lower == "safari" {
        r#"tell application "Safari" to get URL of front document"#.to_string()
    } else {
        // Chrome, Brave, Vivaldi, Edge, Opera, Arc, Chromium all use Chrome's scripting API
        format!(
            r#"tell application "{}" to get URL of active tab of front window"#,
            app_name
        )
    };

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if url.is_empty() || url == "missing value" {
            None
        } else {
            Some(url)
        }
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_browser_url(_app_name: &str, _bundle_id: Option<&str>) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_browser_by_name() {
        assert!(is_browser("Google Chrome", None));
        assert!(is_browser("Safari", None));
        assert!(is_browser("Arc", None));
        assert!(is_browser("Zen Browser", None));
        assert!(!is_browser("Visual Studio Code", None));
    }

    #[test]
    fn test_is_browser_by_bundle_id() {
        assert!(is_browser("Some App", Some("com.google.chrome.canary")));
        assert!(is_browser("Some App", Some("com.apple.safari")));
        assert!(!is_browser("Some App", Some("com.apple.finder")));
    }

    #[test]
    fn test_extract_site_name() {
        assert_eq!(
            extract_site_name("GitHub - Google Chrome"),
            Some("GitHub".to_string())
        );
        assert_eq!(
            extract_site_name("Inbox - Gmail - Arc"),
            Some("Inbox - Gmail".to_string())
        );
        assert_eq!(extract_site_name("Some Random Title"), None);
        assert_eq!(
            extract_site_name("Reddit - Mozilla Firefox"),
            Some("Reddit".to_string())
        );
    }

    #[test]
    fn test_chromium_profile_dir() {
        // Can't check exact path (depends on HOME), but check that known browsers return Some
        // and unknown return None
        assert!(chromium_profile_dir("chrome").is_some());
        assert!(chromium_profile_dir("arc").is_some());
        assert!(chromium_profile_dir("brave").is_some());
        assert!(chromium_profile_dir("edge").is_some());
        assert!(chromium_profile_dir("safari").is_none());
        assert!(chromium_profile_dir("firefox").is_none());
    }
}
