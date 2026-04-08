//! Handles reading, writing, diffing, and hashing skill files on disk.
//!
//! Skills live at `~/.klyntbot/skills/<skill-name>/` as directories containing
//! markdown files: `SKILL.md`, `references/*.md`, `scripts/*.md`, etc.

use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// A single markdown file within a skill directory.
#[derive(Debug, Clone)]
pub struct SkillFile {
    /// The name of the skill (directory name under the skills root).
    pub skill_name: String,
    /// Relative path within the skill directory, e.g. `"SKILL.md"` or `"references/cron.md"`.
    /// Always uses forward slashes.
    pub file_path: String,
    /// Full UTF-8 content of the file.
    pub content: String,
    /// SHA-256 of the content, hex-encoded (first 16 hex chars = 8 bytes).
    pub content_hash: String,
}

/// Manages reading and writing skill files under a given root directory.
pub struct SkillFileManager {
    skills_dir: PathBuf,
}

impl SkillFileManager {
    /// Create a new manager rooted at `skills_dir`.
    pub fn new(skills_dir: PathBuf) -> Self {
        Self { skills_dir }
    }

    /// Returns the root skills directory.
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// Read all skill files from disk.
    ///
    /// Returns a map of `skill_name → Vec<SkillFile>`. Hidden directories
    /// (names starting with `.`) are skipped. Only `.md` files are returned.
    /// Subdirectories (`references/`, `scripts/`, etc.) are scanned recursively.
    ///
    /// Returns an empty map if the skills directory does not exist.
    pub fn read_all(&self) -> HashMap<String, Vec<SkillFile>> {
        let mut result: HashMap<String, Vec<SkillFile>> = HashMap::new();

        let root = &self.skills_dir;
        if !root.exists() {
            return result;
        }

        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => return result,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            // Skip hidden directories.
            if name.starts_with('.') {
                continue;
            }

            let files = collect_md_files(&path, &path, &name);
            if !files.is_empty() {
                result.insert(name, files);
            }
        }

        result
    }

    /// Write `content` to `<skills_dir>/<skill_name>/<file_path>`, creating
    /// any intermediate directories as needed.
    pub fn write_file(&self, skill_name: &str, file_path: &str, content: &str) -> io::Result<()> {
        // Normalise the relative path to the OS separator.
        let rel: PathBuf = file_path.split('/').collect();
        let dest = self.skills_dir.join(skill_name).join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, content)
    }

    /// Seed the skills directory from compiled defaults if it contains no skill
    /// directories yet.
    ///
    /// `defaults` maps `skill_name → Vec<(file_path, content)>`.
    ///
    /// Returns the number of skills written. Returns `0` on a second call
    /// because skill directories already exist.
    pub fn seed_if_empty(
        &self,
        defaults: &HashMap<String, Vec<(&str, &str)>>,
    ) -> io::Result<usize> {
        // Check if any non-hidden directories already exist.
        if self.skills_dir.exists() {
            let has_skills = std::fs::read_dir(&self.skills_dir)?.flatten().any(|e| {
                e.path().is_dir()
                    && e.file_name()
                        .to_str()
                        .map(|n| !n.starts_with('.'))
                        .unwrap_or(false)
            });
            if has_skills {
                return Ok(0);
            }
        }

        let mut count = 0;
        for (skill_name, files) in defaults {
            for (file_path, content) in files {
                self.write_file(skill_name, file_path, content)?;
            }
            count += 1;
        }
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Compute a short SHA-256 hash of `content`.
///
/// Returns the first 16 hex characters (8 bytes) of the digest.
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let full = hex::encode(hasher.finalize());
    full[..16].to_string()
}

/// Produce a unified-style diff between `old` and `new`.
///
/// Each output line is prefixed with `-` (deletion), `+` (insertion), or ` `
/// (context), followed by the line content (without a trailing newline on
/// the prefix line itself).
pub fn compute_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let prefix = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        out.push(prefix);
        out.push_str(change.value());
    }
    out
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively collect all `.md` files under `dir`, recording paths relative
/// to `skill_root`.
fn collect_md_files(dir: &Path, skill_root: &Path, skill_name: &str) -> Vec<SkillFile> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return files,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_md_files(&path, skill_root, skill_name));
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" {
                continue;
            }
            let rel = match path.strip_prefix(skill_root) {
                Ok(r) => r,
                Err(_) => continue,
            };
            // Always use forward slashes in the stored path.
            let file_path = rel.to_string_lossy().replace('\\', "/");
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let hash = content_hash(&content);
            files.push(SkillFile {
                skill_name: skill_name.to_string(),
                file_path,
                content,
                content_hash: hash,
            });
        }
    }
    files
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn test_content_hash_different_for_different_content() {
        assert_ne!(content_hash("hello"), content_hash("world"));
    }

    #[test]
    fn test_compute_diff() {
        let diff = compute_diff("line1\nline2\n", "line1\nline3\n");
        assert!(diff.contains("-line2"));
        assert!(diff.contains("+line3"));
    }

    #[test]
    fn test_read_all_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());
        assert!(mgr.read_all().is_empty());
    }

    #[test]
    fn test_write_and_read() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());
        mgr.write_file("test-skill", "SKILL.md", "---\nname: test\n---\nBody")
            .unwrap();
        mgr.write_file("test-skill", "references/guide.md", "Guide content")
            .unwrap();
        let all = mgr.read_all();
        let files = all.get("test-skill").unwrap();
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|f| f.file_path == "SKILL.md" && f.content.contains("Body")));
        assert!(files
            .iter()
            .any(|f| f.file_path.contains("guide") && f.content.contains("Guide")));
    }

    #[test]
    fn test_seed_if_empty() {
        let tmp = TempDir::new().unwrap();
        let mgr = SkillFileManager::new(tmp.path().to_path_buf());
        let mut defaults = HashMap::new();
        defaults.insert(
            "general".to_string(),
            vec![("SKILL.md", "---\nname: general\n---\nGeneral skill")],
        );
        let count = mgr.seed_if_empty(&defaults).unwrap();
        assert_eq!(count, 1);
        // Second call should skip.
        let count2 = mgr.seed_if_empty(&defaults).unwrap();
        assert_eq!(count2, 0);
    }
}
