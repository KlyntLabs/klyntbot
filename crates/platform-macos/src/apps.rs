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

/// Disk-backed cache for application icons (base64-encoded PNG data URIs).
pub struct AppIconCache {
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

    /// Resolve an app icon, returning `(data_uri, was_cache_hit)`.
    ///
    /// Checks disk cache first; falls back to `sips` extraction on macOS.
    #[cfg(target_os = "macos")]
    pub fn resolve_icon(&self, app_path: &Path, tmp_dir: &Path) -> (Option<String>, bool) {
        let stem = match app_path.file_stem() {
            Some(s) => s.to_string_lossy().replace(' ', "_"),
            None => return (None, false),
        };

        let app_mtime = Self::get_mtime(app_path).unwrap_or(0);

        // Try disk cache
        let cached_png = self.cache_dir.join(format!("{stem}.png"));
        let cached_mtime = self.cache_dir.join(format!("{stem}.mtime"));

        if cached_png.exists() && cached_mtime.exists() {
            if let Ok(stored) = std::fs::read_to_string(&cached_mtime) {
                if stored.trim().parse::<u64>().ok() == Some(app_mtime) {
                    if let Ok(png_bytes) = std::fs::read(&cached_png) {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                        return (Some(format!("data:image/png;base64,{b64}")), true);
                    }
                }
            }
        }

        // Cache miss -- extract via sips
        let data_uri = Self::extract_icon(app_path, tmp_dir);

        // Write to disk cache for next time
        if let Some(ref uri) = data_uri {
            if let Some(b64_data) = uri.strip_prefix("data:image/png;base64,") {
                use base64::Engine;
                if let Ok(png_bytes) = base64::engine::general_purpose::STANDARD.decode(b64_data) {
                    let _ = std::fs::write(self.cache_dir.join(format!("{stem}.png")), &png_bytes);
                    let _ = std::fs::write(
                        self.cache_dir.join(format!("{stem}.mtime")),
                        app_mtime.to_string(),
                    );
                }
            }
        }

        (data_uri, false)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn resolve_icon(&self, _app_path: &Path, _tmp_dir: &Path) -> (Option<String>, bool) {
        (None, false)
    }

    /// Get the modification time of a path as seconds since UNIX epoch.
    #[cfg(target_os = "macos")]
    fn get_mtime(path: &Path) -> Option<u64> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?;
        Some(mtime.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
    }

    /// Extract an app's icon as a base64 data URI.
    ///
    /// Tries `sips` (fast, works for apps with `.icns` files) first, then
    /// falls back to `NSWorkspace.iconForFile:` (handles asset catalogs).
    ///
    /// **Blocking:** Callers in async contexts must use `spawn_blocking`.
    #[cfg(target_os = "macos")]
    fn extract_icon(app_path: &Path, tmp_dir: &Path) -> Option<String> {
        // Fast path: try sips-based extraction (works for .icns apps)
        if let Some(icon) = Self::extract_icon_sips(app_path, tmp_dir) {
            return Some(icon);
        }
        // Fallback: NSWorkspace handles all icon types (asset catalogs, etc.)
        icon_for_path(app_path)
    }

    /// Extract icon via PlistBuddy + sips (traditional .icns approach).
    #[cfg(target_os = "macos")]
    fn extract_icon_sips(app_path: &Path, tmp_dir: &Path) -> Option<String> {
        use std::process::Command;

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

        let stem = app_path.file_stem()?.to_string_lossy().replace(' ', "_");
        let png_path = tmp_dir.join(format!("{stem}.png"));

        let sips_result = Command::new("sips")
            .args([
                "-s",
                "format",
                "png",
                "--resampleWidth",
                "32",
                &icns_path.to_string_lossy(),
                "--out",
                &png_path.to_string_lossy(),
            ])
            .output()
            .ok()?;

        if !sips_result.status.success() {
            return None;
        }

        let png_bytes = std::fs::read(&png_path).ok()?;
        let _ = std::fs::remove_file(&png_path);

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        Some(format!("data:image/png;base64,{b64}"))
    }
}

/// Get icon for any file or application path using NSWorkspace.
///
/// Uses `NSWorkspace.iconForFile:` which handles all icon types: `.icns`,
/// asset catalogs, file type associations, etc. Returns a base64 PNG data URI.
///
/// **Blocking:** Makes AppKit calls — use inside `spawn_blocking`.
#[cfg(target_os = "macos")]
pub fn icon_for_path(path: &Path) -> Option<String> {
    use objc2::msg_send_id;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let path_str = path.to_str()?;
    let ns_path = NSString::from_str(path_str);
    let workspace = NSWorkspace::sharedWorkspace();

    // iconForFile: always returns an NSImage (never nil)
    let image: Retained<AnyObject> =
        unsafe { msg_send_id![&workspace, iconForFile: &*ns_path] };

    nsimage_to_png_data_uri(&image)
}

/// Get system icon for a file type/extension using NSWorkspace.
///
/// Pass a bare extension like `"rs"`, `"py"`, `"txt"`.
///
/// **Blocking:** Makes AppKit calls — use inside `spawn_blocking`.
#[cfg(target_os = "macos")]
pub fn icon_for_file_type(extension: &str) -> Option<String> {
    use objc2::msg_send_id;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let ns_ext = NSString::from_str(extension);
    let workspace = NSWorkspace::sharedWorkspace();

    let image: Retained<AnyObject> =
        unsafe { msg_send_id![&workspace, iconForFileType: &*ns_ext] };

    nsimage_to_png_data_uri(&image)
}

#[cfg(not(target_os = "macos"))]
pub fn icon_for_path(_path: &Path) -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn icon_for_file_type(_extension: &str) -> Option<String> {
    None
}

/// Convert an NSImage (passed as AnyObject) to a base64 PNG data URI.
///
/// Uses TIFFRepresentation → NSBitmapImageRep → PNG conversion.
#[cfg(target_os = "macos")]
fn nsimage_to_png_data_uri(image: &objc2::runtime::AnyObject) -> Option<String> {
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, msg_send_id};

    // Get TIFF representation (includes all image reps)
    let tiff: Option<Retained<AnyObject>> =
        unsafe { msg_send_id![image, TIFFRepresentation] };
    let tiff = tiff?;

    // Create NSBitmapImageRep from TIFF data
    let cls = AnyClass::get(c"NSBitmapImageRep")?;
    let alloc: Allocated<AnyObject> = unsafe { msg_send_id![cls, alloc] };
    let rep: Option<Retained<AnyObject>> =
        unsafe { msg_send_id![alloc, initWithData: &*tiff] };
    let rep = rep?;

    // Convert to PNG — NSBitmapImageFileType::PNG = 4
    let dict_cls = AnyClass::get(c"NSDictionary")?;
    let empty: Retained<AnyObject> = unsafe { msg_send_id![dict_cls, new] };
    let png: Option<Retained<AnyObject>> = unsafe {
        msg_send_id![&rep, representationUsingType: 4usize, properties: &*empty]
    };
    let png = png?;

    // Extract bytes
    let len: usize = unsafe { msg_send![&png, length] };
    let ptr: *const u8 = unsafe { msg_send![&png, bytes] };
    if ptr.is_null() || len == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:image/png;base64,{b64}"))
}
