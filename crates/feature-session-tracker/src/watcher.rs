use crate::parser;
use crate::types::SessionMessage;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Messages emitted by the SessionWatcher.
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// New message parsed from a session file.
    NewMessage {
        session_id: String,
        message: SessionMessage,
    },
    /// A new session file was discovered.
    NewSession {
        session_id: String,
        jsonl_path: PathBuf,
    },
    /// A session file was modified (for status tracking).
    FileModified { session_id: String },
}

/// Watches Claude Code session JSONL files for real-time updates.
pub struct SessionWatcher {
    _watcher: RecommendedWatcher,
    offsets: Arc<Mutex<HashMap<PathBuf, u64>>>,
}

impl SessionWatcher {
    /// Start watching a Claude projects directory.
    /// Returns the watcher and events are sent via `event_tx`.
    pub fn start(
        claude_projects_dir: &Path,
        event_tx: mpsc::UnboundedSender<WatchEvent>,
    ) -> Result<Self, notify::Error> {
        let offsets: Arc<Mutex<HashMap<PathBuf, u64>>> = Arc::new(Mutex::new(HashMap::new()));
        let offsets_clone = offsets.clone();
        let tx = event_tx;

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                let event = match result {
                    Ok(e) => e,
                    Err(e) => {
                        error!("Watch error: {e}");
                        return;
                    }
                };

                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {}
                    _ => return,
                }

                for path in &event.paths {
                    if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }

                    let session_id = match path.file_stem().and_then(|s| s.to_str()) {
                        Some(s) => s.to_string(),
                        None => continue,
                    };

                    if matches!(event.kind, EventKind::Create(_)) {
                        let _ = tx.send(WatchEvent::NewSession {
                            session_id: session_id.clone(),
                            jsonl_path: path.clone(),
                        });
                    }

                    let _ = tx.send(WatchEvent::FileModified {
                        session_id: session_id.clone(),
                    });

                    // Read new lines inline — the notify callback runs on its own
                    // thread pool and read_new_lines is fast (line reads only).
                    let Ok(mut offsets_guard) = offsets_clone.lock() else {
                        warn!("Failed to acquire offsets lock");
                        continue;
                    };
                    if let Some(messages) = read_new_lines(path, &mut offsets_guard) {
                        for msg in messages {
                            let _ = tx.send(WatchEvent::NewMessage {
                                session_id: session_id.clone(),
                                message: msg,
                            });
                        }
                    }
                }
            },
            Config::default(),
        )?;

        watcher.watch(claude_projects_dir, RecursiveMode::Recursive)?;

        info!(
            "Session watcher started on {}",
            claude_projects_dir.display()
        );

        Ok(Self {
            _watcher: watcher,
            offsets,
        })
    }

    /// Initialize offsets for already-known session files (seek to end).
    /// Call this after discovery to avoid replaying existing content.
    pub fn init_offsets(&self, jsonl_paths: &[PathBuf]) {
        let Ok(mut offsets) = self.offsets.lock() else {
            return;
        };
        for path in jsonl_paths {
            if let Ok(metadata) = std::fs::metadata(path) {
                offsets.insert(path.clone(), metadata.len());
            }
        }
    }
}

/// Read new lines from a JSONL file starting at the stored offset.
fn read_new_lines(
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
) -> Option<Vec<SessionMessage>> {
    let file = std::fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();

    let offset = offsets.get(path).copied().unwrap_or(0);

    if file_len <= offset {
        return None;
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset)).ok()?;

    let mut messages = Vec::new();
    let mut new_offset = offset;

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => {
                new_offset += n as u64;
                if let Some(msg) = parser::parse_line(&line) {
                    messages.push(msg);
                }
            }
            Err(e) => {
                warn!("Error reading {}: {e}", path.display());
                break;
            }
        }
    }

    offsets.insert(path.to_path_buf(), new_offset);

    if messages.is_empty() {
        None
    } else {
        Some(messages)
    }
}
