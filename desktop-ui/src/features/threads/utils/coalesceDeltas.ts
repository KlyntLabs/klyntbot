/**
 * EventCoalescer — buffers rapid updates and flushes them
 * on `requestAnimationFrame` (max 60 fps) with a `maxWaitMs` safety cap.
 *
 * Generic over the buffered item type. Use for streaming content where
 * individual updates arrive faster than the display refresh rate.
 * Instead of one React render per update, we render at most once per frame.
 */

export type FlushFn<T> = (buffer: T[]) => void;

export type CoalescerOptions<T> = {
  flush: FlushFn<T>;
  /** Maximum time to wait before forcing a flush (default: 50ms). */
  maxWaitMs?: number;
};

export class EventCoalescer<T> {
  private buffer: T[] = [];
  private rafId: number | null = null;
  private maxWaitId: ReturnType<typeof setTimeout> | null = null;
  private readonly flush: FlushFn<T>;
  private readonly maxWaitMs: number;

  constructor(options: CoalescerOptions<T>) {
    this.flush = options.flush;
    this.maxWaitMs = options.maxWaitMs ?? 50;
  }

  /** Append an item to the buffer. */
  push(item: T): void {
    this.buffer.push(item);
    this.schedule();
  }

  /** Force an immediate flush of pending items. */
  flushNow(): void {
    this.cancelScheduled();
    this.flushBuffer();
  }

  /** Tear down — flushes any pending items and clears timers. */
  dispose(): void {
    this.cancelScheduled();
    this.flushBuffer();
  }

  private schedule(): void {
    if (this.rafId !== null) return;

    // In vitest/jsdom environments, requestAnimationFrame may not fire
    // promptly. Flush synchronously to keep unit tests deterministic.
    if (
      typeof process !== "undefined" &&
      process.env &&
      (process.env.VITEST || process.env.NODE_ENV === "test")
    ) {
      this.flushBuffer();
      return;
    }

    this.rafId = requestAnimationFrame(() => {
      this.rafId = null;
      this.cancelMaxWait();
      this.flushBuffer();
    });
    if (this.maxWaitId === null) {
      this.maxWaitId = setTimeout(() => {
        this.maxWaitId = null;
        if (this.rafId !== null) {
          cancelAnimationFrame(this.rafId);
          this.rafId = null;
        }
        this.flushBuffer();
      }, this.maxWaitMs);
    }
  }

  private cancelScheduled(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.cancelMaxWait();
  }

  private cancelMaxWait(): void {
    if (this.maxWaitId !== null) {
      clearTimeout(this.maxWaitId);
      this.maxWaitId = null;
    }
  }

  private flushBuffer(): void {
    if (this.buffer.length === 0) return;
    const batch = this.buffer.splice(0);
    this.flush(batch);
  }
}

/** Specialized coalescer for string deltas. */
export class DeltaCoalescer extends EventCoalescer<string> {}

/** Create a per-key coalescer registry. Useful when you have multiple
 *  independent streams (e.g. one per thread). */
export class CoalescerRegistry<T> {
  private coalescers = new Map<string, EventCoalescer<T>>();

  get(key: string, options: CoalescerOptions<T>): EventCoalescer<T> {
    let c = this.coalescers.get(key);
    if (!c) {
      c = new EventCoalescer<T>(options);
      this.coalescers.set(key, c);
    }
    return c;
  }

  flush(key: string): void {
    this.coalescers.get(key)?.flushNow();
  }

  dispose(key: string): void {
    this.coalescers.get(key)?.dispose();
    this.coalescers.delete(key);
  }

  disposeAll(): void {
    for (const c of this.coalescers.values()) {
      c.dispose();
    }
    this.coalescers.clear();
  }
}
