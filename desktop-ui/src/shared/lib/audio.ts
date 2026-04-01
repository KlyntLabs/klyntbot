/**
 * Shared AudioContext — reused across calls to avoid autoplay policy issues.
 * Creating a new AudioContext per call can leave it in "suspended" state when
 * there hasn't been a direct user gesture in the WebView (e.g. the voice orb
 * is opened via a global hotkey, not a button click).
 */
let sharedCtx: AudioContext | null = null;

/** Tracks the currently playing TTS source so it can be stopped on interrupt. */
let activeSource: AudioBufferSourceNode | null = null;

function getAudioContext(): AudioContext {
  if (!sharedCtx || sharedCtx.state === "closed") {
    sharedCtx = new AudioContext();
  }
  return sharedCtx;
}

/**
 * Unlock the AudioContext by playing a silent buffer.
 * Call this on a user gesture (click, mount after hotkey) to prime the context
 * before TTS audio needs to play. Safe to call multiple times.
 */
export async function unlockAudioContext(): Promise<void> {
  const ctx = getAudioContext();
  if (ctx.state === "suspended") {
    // Create and play a tiny silent buffer to satisfy the autoplay policy
    const buffer = ctx.createBuffer(1, 1, 22050);
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(ctx.destination);
    source.start();
    try {
      await ctx.resume();
    } catch {
      // Best-effort — some environments block resume entirely
    }
  }
}

/**
 * Stop any currently playing TTS audio immediately.
 * Safe to call when nothing is playing.
 */
export function stopTtsAudio(): void {
  if (activeSource) {
    try {
      activeSource.stop();
    } catch {
      // Already stopped — ignore
    }
    activeSource = null;
  }
}

/**
 * Decode base64-encoded PCM float32 samples and play via Web Audio API.
 * Uses fetch with a data URL instead of atob() for robust binary decoding.
 */
export async function playTtsAudio(base64: string, sampleRate: number): Promise<void> {
  if (!base64) return;

  // Stop any previous playback before starting new audio
  stopTtsAudio();

  const response = await fetch(`data:application/octet-stream;base64,${base64}`);
  const arrayBuffer = await response.arrayBuffer();
  const float32 = new Float32Array(arrayBuffer);

  if (float32.length === 0) return;

  const ctx = getAudioContext();

  // Resume if suspended (autoplay policy blocks audio without user gesture)
  if (ctx.state === "suspended") {
    try {
      await ctx.resume();
    } catch (e) {
      console.error("[TTS] AudioContext.resume() failed:", e);
    }
    if (ctx.state !== "running") {
      console.error("[TTS] AudioContext still suspended after resume, state:", ctx.state);
      return;
    }
  }

  console.log(
    `[TTS] Playing ${float32.length} samples at ${sampleRate}Hz (ctx.state=${ctx.state})`,
  );

  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  const source = ctx.createBufferSource();
  source.buffer = buffer;
  source.connect(ctx.destination);
  source.onended = () => {
    if (activeSource === source) {
      activeSource = null;
    }
  };
  activeSource = source;
  source.start();
}
