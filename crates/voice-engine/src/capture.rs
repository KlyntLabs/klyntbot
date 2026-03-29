//! Audio capture from microphone via cpal.
//!
//! Bridges the cpal audio callback thread to tokio async via channel relay.
//! Provides RMS computation for waveform visualization and silence detection.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::types::AudioChunk;

/// Configuration for audio capture.
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Sample rate in Hz (default: 16000 for Whisper compatibility).
    pub sample_rate: u32,
    /// Number of channels (default: 1 = mono).
    pub channels: u16,
    /// RMS threshold below which audio is considered silence (default: 0.01).
    pub silence_threshold: f32,
    /// Duration of continuous silence before auto-stop (default: 1.5s).
    pub silence_duration: Duration,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            silence_threshold: 0.01,
            silence_duration: Duration::from_millis(1500),
        }
    }
}

/// A running audio capture session.
pub struct CaptureSession {
    /// cpal stream handle -- dropping this stops the audio stream.
    _stream: cpal::Stream,
    /// Signal to stop the capture.
    pub(crate) stop_signal: Arc<AtomicBool>,
    /// Receiver for audio chunks (16kHz mono f32).
    pub audio_rx: mpsc::Receiver<AudioChunk>,
    /// Receiver for RMS levels (~30fps for waveform animation).
    pub rms_rx: mpsc::Receiver<f32>,
    /// Receiver for silence detection events.
    pub silence_rx: mpsc::Receiver<()>,
}

/// Audio capture subsystem wrapping cpal.
pub struct AudioCapture {
    config: CaptureConfig,
}

impl AudioCapture {
    pub fn new(config: CaptureConfig) -> Self {
        Self { config }
    }

    /// List available input devices.
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }

    /// Start capturing audio from the default input device.
    ///
    /// Returns a `CaptureSession` with receivers for audio chunks, RMS levels,
    /// and silence detection. The capture runs until the session is dropped
    /// or `stop()` is called.
    pub fn start(&self) -> common::Result<CaptureSession> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or_else(|| {
            common::KlyntbotError::Channel(common::ChannelError::ConnectionFailed(
                "No audio input device found".to_string(),
            ))
        })?;

        let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());
        info!("Opening audio input: {}", device_name);

        // Use the device's default config — most macOS mics don't support 16kHz directly.
        // We capture at the native rate and downsample to 16kHz for Whisper.
        let default_config = device.default_input_config().map_err(|e| {
            common::KlyntbotError::Channel(common::ChannelError::ConnectionFailed(
                format!("Failed to get default input config: {}", e),
            ))
        })?;

        let native_sample_rate = default_config.sample_rate().0;
        let native_channels = default_config.channels();
        info!(
            "Device native config: {}Hz, {} ch (target: {}Hz)",
            native_sample_rate, native_channels, self.config.sample_rate
        );

        let stream_config = cpal::StreamConfig {
            channels: native_channels,
            sample_rate: default_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        // Channels for bridging cpal callback thread -> tokio
        let (audio_tx, audio_rx) = mpsc::channel::<AudioChunk>(64);
        let (rms_tx, rms_rx) = mpsc::channel::<f32>(32);
        let (silence_tx, silence_rx) = mpsc::channel::<()>(1);

        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_cb = stop_signal.clone();

        let silence_threshold = self.config.silence_threshold;
        let silence_duration = self.config.silence_duration;
        let target_sample_rate = self.config.sample_rate;
        let downsample_ratio = native_sample_rate / target_sample_rate;
        let num_channels = native_channels;

        // State for silence detection (lives in the audio callback thread)
        let mut last_voice_time = Instant::now();
        let mut silence_fired = false;
        let mut rms_counter: u32 = 0;

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    if stop_signal_cb.load(Ordering::Relaxed) {
                        return;
                    }

                    let rms = compute_rms(data);

                    // Silence detection on every callback
                    if rms > silence_threshold {
                        last_voice_time = Instant::now();
                        silence_fired = false;
                    } else if !silence_fired && last_voice_time.elapsed() >= silence_duration {
                        let _ = silence_tx.try_send(());
                        silence_fired = true;
                    }

                    // RMS emission throttled to ~30fps (based on native sample rate)
                    rms_counter += data.len() as u32;
                    if rms_counter >= native_sample_rate / 30 {
                        let _ = rms_tx.try_send(rms);
                        rms_counter = 0;
                    }

                    // Downsample to 16kHz mono for Whisper:
                    // 1) Mix to mono (average channels)
                    // 2) Skip samples to match target rate (simple decimation)
                    let mono: Vec<f32> = if num_channels > 1 {
                        data.chunks(num_channels as usize)
                            .map(|frame| frame.iter().sum::<f32>() / num_channels as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };

                    let downsampled: Vec<f32> = if downsample_ratio > 1 {
                        mono.iter()
                            .step_by(downsample_ratio as usize)
                            .copied()
                            .collect()
                    } else {
                        mono
                    };

                    let chunk = AudioChunk {
                        samples: downsampled,
                        sample_rate: target_sample_rate,
                    };
                    let _ = audio_tx.try_send(chunk);
                },
                move |err| {
                    error!("Audio capture error: {}", err);
                },
                None,
            )
            .map_err(|e| {
                common::KlyntbotError::Channel(common::ChannelError::ConnectionFailed(format!(
                    "Failed to build audio stream: {}",
                    e
                )))
            })?;

        stream.play().map_err(|e| {
            common::KlyntbotError::Channel(common::ChannelError::ConnectionFailed(format!(
                "Failed to start audio stream: {}",
                e
            )))
        })?;

        debug!(
            "Audio capture started: {}Hz, {} ch",
            self.config.sample_rate, self.config.channels
        );

        Ok(CaptureSession {
            _stream: stream,
            stop_signal,
            audio_rx,
            rms_rx,
            silence_rx,
        })
    }
}

impl CaptureSession {
    /// Signal the capture to stop.
    pub fn stop(&self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }

    /// Check if capture has been signaled to stop.
    pub fn is_stopped(&self) -> bool {
        self.stop_signal.load(Ordering::Relaxed)
    }
}

/// Compute RMS (root mean square) of an audio buffer.
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = CaptureConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.channels, 1);
        assert_eq!(config.silence_duration, Duration::from_millis(1500));
    }

    #[test]
    fn compute_rms_silence() {
        assert_eq!(compute_rms(&[0.0; 100]), 0.0);
    }

    #[test]
    fn compute_rms_signal() {
        let rms = compute_rms(&[1.0; 100]);
        assert!((rms - 1.0).abs() < 0.001);
    }

    #[test]
    fn compute_rms_empty() {
        assert_eq!(compute_rms(&[]), 0.0);
    }

    #[test]
    fn compute_rms_mixed() {
        // RMS of [0.5, -0.5, 0.5, -0.5] = sqrt(0.25) = 0.5
        let rms = compute_rms(&[0.5, -0.5, 0.5, -0.5]);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn list_devices_does_not_panic() {
        // Just verify it doesn't crash -- may return empty on CI
        let _ = AudioCapture::list_devices();
    }
}
