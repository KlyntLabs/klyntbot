/**
 * Decode base64-encoded PCM float32 samples and play via Web Audio API.
 * Uses fetch with a data URL instead of atob() for robust binary decoding.
 */
export async function playTtsAudio(base64: string, sampleRate: number): Promise<void> {
  if (!base64) return;

  const response = await fetch(`data:application/octet-stream;base64,${base64}`);
  const arrayBuffer = await response.arrayBuffer();
  const float32 = new Float32Array(arrayBuffer);

  if (float32.length === 0) return;

  const ctx = new AudioContext();
  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}
