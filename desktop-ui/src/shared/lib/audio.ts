/**
 * Decode base64-encoded PCM float32 samples and play via Web Audio API.
 * Used for TTS playback in the Voice Brain orb.
 */
export function playTtsAudio(base64: string, sampleRate: number): void {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }

  // PCM data is little-endian float32
  const float32 = new Float32Array(bytes.buffer);

  const ctx = new AudioContext();
  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}
