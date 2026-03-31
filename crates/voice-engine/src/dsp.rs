//! Audio DSP pipeline: noise reduction + anti-aliased downsampling.

/// Downsample audio from `src_rate` to `dst_rate` with a simple low-pass
/// averaging filter to prevent aliasing. Replaces naive `step_by` decimation.
pub fn downsample_with_filter(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate {
        return samples.to_vec();
    }

    let ratio = src_rate as f64 / dst_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    // Simple averaging decimation: for each output sample, average the
    // surrounding input samples within the decimation window.
    // This acts as a low-pass filter at dst_rate/2 Hz.
    let window = ratio.ceil() as usize;

    for i in 0..output_len {
        let center = (i as f64 * ratio) as usize;
        let start = center.saturating_sub(window / 2);
        let end = (center + window / 2 + 1).min(samples.len());
        let sum: f32 = samples[start..end].iter().sum();
        let count = (end - start) as f32;
        output.push(sum / count);
    }

    output
}

/// Apply noise reduction via nnnoiseless (RNNoise).
/// Operates on 48kHz mono audio in frames of 480 samples.
#[cfg(feature = "vad")]
pub fn denoise_48khz(samples: &[f32]) -> Vec<f32> {
    use nnnoiseless::DenoiseState;

    let mut state = DenoiseState::new();
    let mut output = Vec::with_capacity(samples.len());
    let mut frame_buf = [0.0f32; DenoiseState::FRAME_SIZE];

    for chunk in samples.chunks(DenoiseState::FRAME_SIZE) {
        frame_buf[..chunk.len()].copy_from_slice(chunk);
        if chunk.len() < DenoiseState::FRAME_SIZE {
            frame_buf[chunk.len()..].fill(0.0);
        }
        let mut out_frame = [0.0f32; DenoiseState::FRAME_SIZE];
        state.process_frame(&mut out_frame, &frame_buf);
        output.extend_from_slice(&out_frame[..chunk.len()]);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = downsample_with_filter(&input, 16000, 16000);
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn downsample_48k_to_16k() {
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.01).sin()).collect();
        let output = downsample_with_filter(&input, 48000, 16000);
        assert!((output.len() as i32 - 1600).abs() <= 1);
    }

    #[test]
    fn downsample_reduces_aliasing() {
        let src_rate = 48000.0;
        let input: Vec<f32> = (0..4800)
            .map(|i| {
                let t = i as f32 / src_rate;
                (2.0 * std::f32::consts::PI * 1000.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 20000.0 * t).sin()
            })
            .collect();

        let output = downsample_with_filter(&input, 48000, 16000);

        let naive: Vec<f32> = input.iter().step_by(3).copied().collect();
        let naive_energy: f32 = naive.iter().map(|s| s * s).sum();
        let filtered_energy: f32 = output.iter().map(|s| s * s).sum();

        assert!(
            filtered_energy < naive_energy,
            "Filtered energy ({filtered_energy}) should be less than naive ({naive_energy})"
        );
    }

    #[cfg(feature = "vad")]
    #[test]
    fn denoise_preserves_length() {
        let input = vec![0.01; 4800];
        let output = denoise_48khz(&input);
        assert_eq!(output.len(), input.len());
    }
}
