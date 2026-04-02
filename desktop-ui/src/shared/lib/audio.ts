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
 * Unlock the AudioContext by playing a silent buffer and calling resume().
 * In Tauri, the voice orb opens via a global hotkey — the Rust side calls
 * set_focus() which grants native window focus, allowing ctx.resume() to
 * succeed even without a DOM-level user gesture.
 *
 * Safe to call multiple times.
 */
export async function unlockAudioContext(): Promise<void> {
  const ctx = getAudioContext();
  if (ctx.state === "suspended") {
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

  await playBase64Audio(base64, sampleRate);
}

/** Shared decode + play logic used by both playTtsAudio and the streaming queue. */
async function playBase64Audio(base64: string, sampleRate: number): Promise<void> {
  const response = await fetch(`data:application/octet-stream;base64,${base64}`);
  const arrayBuffer = await response.arrayBuffer();
  const float32 = new Float32Array(arrayBuffer);

  if (float32.length === 0) return;

  const ctx = getAudioContext();

  // Resume if suspended (autoplay policy blocks audio without user gesture)
  if (ctx.state === "suspended") {
    await unlockAudioContext();
    if (ctx.state !== "running") {
      console.error("[TTS] AudioContext still suspended after unlock, state:", ctx.state);
      return;
    }
  }

  const buffer = ctx.createBuffer(1, float32.length, sampleRate);
  buffer.copyToChannel(float32, 0);

  return new Promise<void>((resolve) => {
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(ctx.destination);
    source.onended = () => {
      if (activeSource === source) {
        activeSource = null;
      }
      resolve();
    };
    activeSource = source;
    source.start();
  });
}

// ── Streaming TTS audio queue ───────────────────────────────────────────────

interface AudioChunk {
  base64: string;
  sampleRate: number;
}

/** Queue of audio chunks waiting to be played in order. */
const chunkQueue: AudioChunk[] = [];
let queuePlaying = false;

/**
 * Enqueue a streaming TTS audio chunk for sequential playback.
 * Chunks play in FIFO order — each chunk starts after the previous finishes.
 */
export function enqueueTtsChunk(base64: string, sampleRate: number): void {
  if (!base64) return;
  chunkQueue.push({ base64, sampleRate });
  if (!queuePlaying) {
    drainQueue();
  }
}

/** Process the chunk queue sequentially. */
async function drainQueue(): Promise<void> {
  queuePlaying = true;
  while (chunkQueue.length > 0) {
    const chunk = chunkQueue.shift();
    if (!chunk) break;
    await playBase64Audio(chunk.base64, chunk.sampleRate);
  }
  queuePlaying = false;
}

/** Clear the streaming audio queue and stop current playback. */
export function clearTtsQueue(): void {
  chunkQueue.length = 0;
  queuePlaying = false;
  stopTtsAudio();
}
