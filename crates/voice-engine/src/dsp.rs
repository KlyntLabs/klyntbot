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
/// `state` must persist across calls for the GRU recurrent context to work.
#[cfg(feature = "vad")]
pub fn denoise_48khz(state: &mut nnnoiseless::DenoiseState, samples: &[f32]) -> Vec<f32> {
    let mut output = Vec::with_capacity(samples.len());
    let mut frame_buf = [0.0f32; nnnoiseless::DenoiseState::FRAME_SIZE];

    for chunk in samples.chunks(nnnoiseless::DenoiseState::FRAME_SIZE) {
        frame_buf[..chunk.len()].copy_from_slice(chunk);
        if chunk.len() < nnnoiseless::DenoiseState::FRAME_SIZE {
            frame_buf[chunk.len()..].fill(0.0);
        }
        let mut out_frame = [0.0f32; nnnoiseless::DenoiseState::FRAME_SIZE];
        state.process_frame(&mut out_frame, &frame_buf);
        output.extend_from_slice(&out_frame[..chunk.len()]);
    }

    output
}

/// Encode f32 samples as a WAV byte buffer (16-bit PCM mono).
pub fn encode_wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let file_len = 36 + data_len;
    let mut buf = Vec::with_capacity(44 + samples.len() * 2);

    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_len.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());

    for &s in samples {
        let i16_sample = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        buf.extend_from_slice(&i16_sample.to_le_bytes());
    }

    buf
}

/// Decode 16-bit PCM LE bytes to f32 samples.
pub fn decode_pcm_16bit(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / i16::MAX as f32
        })
        .collect()
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
        let mut state = nnnoiseless::DenoiseState::new();
        let output = denoise_48khz(&mut state, &input);
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn wav_encoding_produces_valid_header() {
        let samples = vec![0.0f32; 100];
        let wav = encode_wav_bytes(&samples, 16000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 200);
    }

    #[test]
    fn pcm_16bit_roundtrip() {
        let original = vec![0.5, -0.5, 0.0, 1.0, -1.0];
        let wav = encode_wav_bytes(&original, 16000);
        let decoded = decode_pcm_16bit(&wav[44..]); // skip header
        assert_eq!(decoded.len(), original.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 0.001, "{a} vs {b}");
        }
    }
}
