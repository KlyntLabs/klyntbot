//! Running application listing and icon caching for macOS.

use std::path::{Path, PathBuf};

/// A running application visible in the Dock.
#[derive(Debug, Clone)]
pub struct RunningApp {
    pub name: String,
    pub bundle_id: Option<String>,
    pub pid: i32,
    pub path: Option<PathBuf>,
}

/// List all regular (non-background) running applications.
#[cfg(target_os = "macos")]
pub fn running_applications() -> Vec<RunningApp> {
    use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};

    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    let mut result = Vec::new();

    for app in &apps {
        if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
            continue;
        }

        let name = match app.localizedName() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
        let pid = app.processIdentifier();
        let path = app.bundleURL().map(|url| {
            let path_str = url.path().map(|p| p.to_string());
            match path_str {
                Some(p) => PathBuf::from(p),
                None => PathBuf::new(),
            }
        });

        result.push(RunningApp {
            name,
            bundle_id,
            pid,
            path,
        });
    }

    result
}

#[cfg(not(target_os = "macos"))]
pub fn running_applications() -> Vec<RunningApp> {
    Vec::new()
}

/// Disk-backed cache for application icons stored as PNG files.
pub struct AppIconCache {
    #[allow(dead_code)]
    cache_dir: PathBuf,
}

impl AppIconCache {
    /// Create a new icon cache at the given directory. Creates the directory if needed.
    pub fn new(cache_dir: PathBuf) -> Self {
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::warn!("Failed to create icon cache dir {:?}: {}", cache_dir, e);
        }
        Self { cache_dir }
    }

    /// Resolve an app icon, returning a path to a cached PNG file.
    ///
    /// Checks disk cache first (validated by app mtime); on miss reads the
    /// app's `Info.plist` via PlistBuddy to locate the `.icns` file, then
    /// converts it to a 64×64 PNG via `sips`. Never calls NSWorkspace.
    ///
    /// **Blocking:** Spawns subprocesses — callers in async contexts must use
    /// `spawn_blocking`.
    #[cfg(target_os = "macos")]
    pub fn resolve_icon_path(&self, app_path: &Path) -> Option<PathBuf> {
        use std::process::Command;

        let stem = app_path.file_stem()?.to_string_lossy().replace(' ', "_");
        let app_mtime = Self::get_mtime(app_path).unwrap_or(0);

        let cached_png = self.cache_dir.join(format!("{stem}.png"));
        let cached_mtime_file = self.cache_dir.join(format!("{stem}.mtime"));

        // Return cached PNG if mtime still matches
        if cached_png.exists() && cached_mtime_file.exists() {
            if let Ok(stored) = std::fs::read_to_string(&cached_mtime_file) {
                if stored.trim().parse::<u64>().ok() == Some(app_mtime) {
                    return Some(cached_png);
                }
            }
        }

        // Read CFBundleIconFile from Info.plist via PlistBuddy
        let plist_path = app_path.join("Contents/Info.plist");
        let output = Command::new("/usr/libexec/PlistBuddy")
            .args([
                "-c",
                "Print :CFBundleIconFile",
                &plist_path.to_string_lossy(),
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let mut icon_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !icon_name.ends_with(".icns") {
            icon_name.push_str(".icns");
        }

        let icns_path = app_path.join("Contents/Resources").join(&icon_name);
        if !icns_path.exists() {
            return None;
        }

        // Convert .icns → PNG via sips (no NSWorkspace involved)
        let sips_result = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                "--resampleWidth",
                "64",
                &icns_path.to_string_lossy(),
                "--out",
                &cached_png.to_string_lossy(),
            ])
            .output()
            .ok()?;

        if !sips_result.status.success() {
            return None;
        }

        // Write mtime so subsequent calls hit the cache
        let _ = std::fs::write(&cached_mtime_file, app_mtime.to_string());

        Some(cached_png)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn resolve_icon_path(&self, _app_path: &Path) -> Option<PathBuf> {
        None
    }

    /// Get the modification time of a path as seconds since UNIX epoch.
    #[allow(dead_code)]
    fn get_mtime(path: &Path) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?;
        Some(mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
    }
}

/// Activate (bring to front) a running application by PID.
/// Returns true if the app was successfully activated, false if not found or failed.
#[cfg(target_os = "macos")]
pub fn activate_app(pid: i32) -> bool {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid);
    match app {
        #[allow(deprecated)]
        Some(app) => {
            app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
        }
        None => false,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn activate_app(_pid: i32) -> bool {
    false
}

/// Read `CFBundleIdentifier` from an app bundle's `Info.plist` via PlistBuddy.
///
/// Returns `None` if:
/// - The app bundle has no `Contents/Info.plist`
/// - The plist exists but has no `CFBundleIdentifier` key
/// - PlistBuddy fails for any other reason (malformed plist, permission, …)
///
/// This mirrors the PlistBuddy approach used by `AppIconCache::resolve_icon_path`
/// — keeps the call out of NSWorkspace / CFBundle Objective-C bridges, so it
/// will not contribute to the IconServices mmap leak.
#[cfg(target_os = "macos")]
pub fn read_bundle_id(app_path: &Path) -> Option<String> {
    use std::process::Command;

    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }

    let output = Command::new("/usr/libexec/PlistBuddy")
        .args([
            "-c",
            "Print :CFBundleIdentifier",
            &plist_path.to_string_lossy(),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let bid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if bid.is_empty() {
        None
    } else {
        Some(bid)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_bundle_id(_app_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod bundle_id_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_fake_app(dir: &Path, name: &str, bundle_id: Option<&str>) -> PathBuf {
        let app_path = dir.join(format!("{name}.app"));
        let contents = app_path.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        let plist = match bundle_id {
            Some(bid) => format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n\
                 <dict>\n\
                 \t<key>CFBundleIdentifier</key>\n\
                 \t<string>{bid}</string>\n\
                 </dict>\n\
                 </plist>\n"
            ),
            None => "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                     <plist version=\"1.0\"><dict></dict></plist>\n".to_string(),
        };
        fs::write(contents.join("Info.plist"), plist).unwrap();
        app_path
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn reads_bundle_id_from_plist() {
        let dir = TempDir::new().unwrap();
        let app = make_fake_app(dir.path(), "Foo", Some("com.example.Foo"));
        assert_eq!(read_bundle_id(&app), Some("com.example.Foo".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn returns_none_when_plist_lacks_identifier() {
        let dir = TempDir::new().unwrap();
        let app = make_fake_app(dir.path(), "NoBundle", None);
        assert_eq!(read_bundle_id(&app), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn returns_none_when_plist_missing() {
        let dir = TempDir::new().unwrap();
        let app = dir.path().join("Empty.app");
        std::fs::create_dir_all(app.join("Contents")).unwrap();
        // No Info.plist written
        assert_eq!(read_bundle_id(&app), None);
    }
}
