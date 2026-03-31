/**
 * Shared AudioContext — reused across calls to avoid autoplay policy issues.
 * Creating a new AudioContext per call can leave it in "suspended" state when
 * there hasn't been a direct user gesture in the WebView (e.g. the voice orb
 * is opened via a global hotkey, not a button click).
 */
let sharedCtx: AudioContext | null = null;

function getAudioContext(): AudioContext {
  if (!sharedCtx || sharedCtx.state === "closed") {
    sharedCtx = new AudioContext();
  }
  return sharedCtx;
}

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

  const ctx = getAudioContext();
  // Resume if suspended (autoplay policy blocks audio without user gesture)
  if (ctx.state === "suspended") {
    console.log("[TTS] AudioContext suspended, resuming...");
    await ctx.resume();
  }

  console.log(`[TTS] Playing ${float32.length} samples at ${sampleRate}Hz (ctx.state=${ctx.state})`);

  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.start();
}
