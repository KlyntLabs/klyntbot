//! Managed-block markdown parser/writer.
//!
//! - **Invariant:** bytes outside `<!-- klyntbot:managed:start ... -->` and
//!   `<!-- klyntbot:managed:end -->` are byte-equal before and after a write.
//! - **Atomic:** writes go through `tempfile::NamedTempFile::persist`.
//! - **Conflict detection:** an optional `prior_inside_hash` lets the caller
//!   reject writes when the user edited inside the managed range between cycles.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use thiserror::Error;

const START_PREFIX: &str = "<!-- klyntbot:managed:start";
const END_MARKER: &str = "<!-- klyntbot:managed:end -->";

/// Managed-block error surface.
#[derive(Debug, Error)]
pub enum ManagedBlockError {
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// User edited managed range — refuse to overwrite.
    #[error("user content found inside managed range — refusing to overwrite")]
    UserConflict,
    /// Markers malformed (start without end, etc.).
    #[error("malformed managed block: {0}")]
    Malformed(String),
}

/// Parsed managed block.
#[derive(Debug, Clone, Default)]
pub struct ManagedBlock {
    /// Lines before the managed start marker (preserved verbatim, includes trailing newline).
    pub before: String,
    /// The original `<!-- klyntbot:managed:start ... -->` marker line (preserved when re-writing).
    pub start_marker: String,
    /// Body inside the managed range (excluding markers).
    pub inside: String,
    /// Lines after the managed end marker (preserved verbatim).
    pub after: String,
}

impl ManagedBlock {
    /// Read + parse a file. Returns a default empty block if the file does not exist.
    pub fn read(path: &Path) -> Result<Self, ManagedBlockError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse a string.
    pub fn parse(content: &str) -> Result<Self, ManagedBlockError> {
        let Some(start_idx) = content.find(START_PREFIX) else {
            // No managed block — entire file is "before" content from the
            // perspective of a fresh write. We treat it as if there is no
            // existing managed range.
            return Ok(ManagedBlock {
                before: content.to_string(),
                start_marker: String::new(),
                inside: String::new(),
                after: String::new(),
            });
        };
        let after_start = &content[start_idx..];
        let Some(start_line_end) = after_start.find('\n') else {
            return Err(ManagedBlockError::Malformed(
                "start marker without newline".into(),
            ));
        };
        let start_marker_line = &after_start[..=start_line_end];
        let body_start = start_idx + start_line_end + 1;
        let Some(end_idx_rel) = content[body_start..].find(END_MARKER) else {
            return Err(ManagedBlockError::Malformed(
                "managed start without matching end marker".into(),
            ));
        };
        let end_idx = body_start + end_idx_rel;
        let inside = &content[body_start..end_idx];
        let after_end = end_idx + END_MARKER.len();
        // Skip the trailing newline of the end marker line if present.
        let after_start_idx = if content[after_end..].starts_with('\n') {
            after_end + 1
        } else {
            after_end
        };
        Ok(ManagedBlock {
            before: content[..start_idx].to_string(),
            start_marker: start_marker_line.to_string(),
            inside: inside.to_string(),
            after: content[after_start_idx..].to_string(),
        })
    }

    /// SHA-256 hex of the inside body.
    #[must_use]
    pub fn inside_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.inside.as_bytes());
        hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
    }

    /// Atomic write: replace the inside body with `new_body` and stamp `cycle_id` in the start marker.
    /// `cycle_id` ends up in the marker comment as `cycle: <id>`.
    pub fn write_with_new_inside(
        &self,
        path: &Path,
        new_body: &str,
        cycle_id: &str,
    ) -> Result<(), ManagedBlockError> {
        self.write_with_new_inside_if_unchanged(path, new_body, cycle_id, None)
    }

    /// Like `write_with_new_inside`, but refuses if the parsed inside hash
    /// differs from `prior_inside_hash` (when supplied).
    pub fn write_with_new_inside_if_unchanged(
        &self,
        path: &Path,
        new_body: &str,
        cycle_id: &str,
        prior_inside_hash: Option<&str>,
    ) -> Result<(), ManagedBlockError> {
        if let Some(prior) = prior_inside_hash {
            if !self.inside.is_empty() && self.inside_hash() != prior {
                return Err(ManagedBlockError::UserConflict);
            }
        }
        let now = jiff::Timestamp::now().to_string();
        let new_marker = format!("{START_PREFIX} | generated: {now} | cycle: {cycle_id} -->\n");
        let mut rebuilt = String::with_capacity(
            self.before.len()
                + new_marker.len()
                + new_body.len()
                + END_MARKER.len()
                + self.after.len()
                + 8,
        );
        rebuilt.push_str(&self.before);
        if !rebuilt.ends_with('\n') && !rebuilt.is_empty() {
            rebuilt.push('\n');
        }
        rebuilt.push_str(&new_marker);
        rebuilt.push_str(new_body);
        if !new_body.ends_with('\n') {
            rebuilt.push('\n');
        }
        rebuilt.push_str(END_MARKER);
        rebuilt.push('\n');
        rebuilt.push_str(&self.after);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent)?;
        tmp.write_all(rebuilt.as_bytes())?;
        tmp.flush()?;
        tmp.persist(path)
            .map_err(|e| ManagedBlockError::Io(e.error))?;
        Ok(())
    }
}
