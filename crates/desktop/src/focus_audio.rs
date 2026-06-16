//! Focus alert audio manager.
//!
//! Plays embedded default MP3s or user-supplied custom files.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use rodio::{Decoder, OutputStream, Sink};
use tauri::{AppHandle, Manager};
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusCue {
    WorkComplete,
    BreakComplete,
}

impl FocusCue {
    fn embedded_bytes(&self) -> &'static [u8] {
        match self {
            FocusCue::WorkComplete => include_bytes!("../assets/audio/focus-work-complete.mp3"),
            FocusCue::BreakComplete => include_bytes!("../assets/audio/focus-break-complete.mp3"),
        }
    }

    pub fn default_filename(&self) -> &'static str {
        match self {
            FocusCue::WorkComplete => "focus-work-complete.mp3",
            FocusCue::BreakComplete => "focus-break-complete.mp3",
        }
    }
}

pub struct FocusAudioManager {
    app_data_dir: PathBuf,
}

impl FocusAudioManager {
    pub fn new(app: &AppHandle) -> Self {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        Self { app_data_dir }
    }

    pub fn play(&self, cue: FocusCue, volume: f32) {
        let path = self.app_data_dir.join("audio").join(cue.default_filename());
        let bytes = if path.exists() {
            match fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    warn!("Failed to read custom focus audio {:?}: {e}", path);
                    cue.embedded_bytes().to_vec()
                }
            }
        } else {
            cue.embedded_bytes().to_vec()
        };

        std::thread::spawn(move || {
            if let Err(e) = play_bytes(bytes, volume) {
                warn!("Failed to play focus audio: {e}");
            }
        });
    }
}

fn play_bytes(bytes: Vec<u8>, volume: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let source = Decoder::new(Cursor::new(bytes))?;
    sink.set_volume(volume.clamp(0.0, 1.0));
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
