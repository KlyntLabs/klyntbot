//! Voice Activity Detection.
//!
//! When the `vad` feature is enabled, uses WebRTC VAD (GMM-based) for
//! speech detection. Falls back to simple RMS thresholding otherwise.
//! Both processors operate on 16kHz mono audio.

/// VAD decision for a chunk of audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VadDecision {
    /// Speech detected. Value is the RMS energy level of the chunk.
    Speech(f32),
    /// Silence detected.
    Silence,
}

/// WebRTC VAD processor using GMM-based speech detection.
///
/// Operates on 16kHz audio in frames of 160 samples (10ms), 320 (20ms),
/// or 480 (30ms). We use 480-sample frames for lower CPU overhead.
#[cfg(feature = "vad")]
pub struct WebrtcVadProcessor {
    vad: webrtc_vad::Vad,
}

// SAFETY: `webrtc_vad::Vad` wraps a `*mut Fvad` raw pointer. `libfvad`
// uses no thread-local state; the pointer is exclusively owned by this
// struct (no aliased mutable access). Moving it to another thread is
// safe as long as concurrent access is prevented — enforced by the
// `Mutex` wrapper in `capture.rs`.
#[cfg(feature = "vad")]
unsafe impl Send for WebrtcVadProcessor {}

#[cfg(feature = "vad")]
impl WebrtcVadProcessor {
    /// Create a new WebRTC VAD processor.
    ///
    /// `aggressive` controls mode: false = Quality (fewer false positives),
    /// true = VeryAggressive (catches more speech in noisy environments).
    pub fn new(aggressive: bool) -> Self {
        let mode = if aggressive {
            webrtc_vad::VadMode::VeryAggressive
        } else {
            webrtc_vad::VadMode::Quality
        };
        Self {
            vad: webrtc_vad::Vad::new_with_rate_and_mode(webrtc_vad::SampleRate::Rate16kHz, mode),
        }
    }

    /// Process a chunk of 16kHz mono f32 audio.
    /// Internally converts to i16 for the WebRTC VAD C API.
    /// For best results, pass 480 samples (30ms) at a time.
    pub fn process_chunk(&mut self, samples: &[f32]) -> VadDecision {
        // Stack-allocate for the fixed 480-sample VAD frame size
        let mut i16_samples = [0i16; 480];
        for (dst, &src) in i16_samples.iter_mut().zip(samples.iter()) {
            *dst = (src * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }

        match self.vad.is_voice_segment(&i16_samples) {
            Ok(true) => {
                let rms = crate::capture::compute_rms(samples);
                VadDecision::Speech(rms)
            }
            _ => VadDecision::Silence,
        }
    }

    /// Reset VAD state (call between sessions).
    pub fn reset(&mut self) {
        self.vad.reset();
    }
}

/// Fallback RMS-based VAD when WebRTC VAD is not available.
pub struct RmsVadProcessor {
    threshold: f32,
}

impl RmsVadProcessor {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }

    pub fn process_chunk(&self, samples: &[f32]) -> VadDecision {
        let rms = crate::capture::compute_rms(samples);
        if rms > self.threshold {
            VadDecision::Speech(rms)
        } else {
            VadDecision::Silence
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_vad_detects_silence() {
        let vad = RmsVadProcessor::new(0.01);
        let silence = vec![0.001; 512];
        assert_eq!(vad.process_chunk(&silence), VadDecision::Silence);
    }

    #[test]
    fn rms_vad_detects_speech() {
        let vad = RmsVadProcessor::new(0.01);
        let speech = vec![0.1; 512];
        match vad.process_chunk(&speech) {
            VadDecision::Speech(_) => {}
            other => panic!("Expected Speech, got {other:?}"),
        }
    }

    #[cfg(feature = "vad")]
    #[test]
    fn webrtc_vad_detects_silence() {
        let mut vad = WebrtcVadProcessor::new(false);
        // 30ms of digital silence at 16kHz
        let silence = vec![0.0; 480];
        assert_eq!(vad.process_chunk(&silence), VadDecision::Silence);
    }

    #[cfg(feature = "vad")]
    #[test]
    fn webrtc_vad_reset_does_not_panic() {
        let mut vad = WebrtcVadProcessor::new(true);
        vad.reset();
        let silence = vec![0.0; 480];
        assert_eq!(vad.process_chunk(&silence), VadDecision::Silence);
    }
}
