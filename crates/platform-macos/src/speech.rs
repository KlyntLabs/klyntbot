//! macOS speech synthesis wrapper using the `say` CLI command.
//!
//! Uses `say` as a pragmatic v1 approach — avoids complex objc2 block
//! wiring and gives immediate access to all macOS neural voices.

use std::path::Path;
use std::process::Command;

use tracing::debug;

/// Info about an available macOS speech voice.
pub struct MacVoice {
    pub name: String,
    pub language: String,
}

/// List available macOS speech voices.
pub fn list_voices() -> Vec<MacVoice> {
    let output = Command::new("say").arg("--voice=?").output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(MacVoice {
                        name: parts[0].to_string(),
                        language: parts[1].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Synthesize text to an audio file using macOS `say` command.
///
/// The output is a WAV file with 16kHz float32 samples suitable for Web Audio API.
pub fn synthesize_to_file(
    text: &str,
    voice: Option<&str>,
    rate: Option<f32>,
    output_path: &Path,
) -> Result<(), String> {
    let mut cmd = Command::new("say");

    if let Some(voice_name) = voice {
        cmd.arg("-v").arg(voice_name);
    }

    if let Some(r) = rate {
        let wpm = (175.0 * r) as u32;
        cmd.arg("-r").arg(wpm.to_string());
    }

    cmd.arg("-o")
        .arg(output_path)
        .arg("--data-format=LEF32@16000")
        .arg(text);

    debug!("Running TTS: say -> {}", output_path.display());

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run say: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("say failed: {}", stderr));
    }

    Ok(())
}
