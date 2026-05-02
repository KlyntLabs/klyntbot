// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.

const DEFAULT_TTL_MS = 300_000;
const MAX_ENTRIES = 100;

interface CacheEntry<T> {
  value: T;
  expiresAt: number;
  promise?: Promise<T>;
}

class ApiCache {
  private store = new Map<string, CacheEntry<unknown>>();

  invalidate(key: string): void {
    this.store.delete(key);
  }

  clear(): void {
    this.store.clear();
  }

  get<T>(
    key: string,
    fetcher: () => Promise<T>,
    ttlMs = DEFAULT_TTL_MS,
  ): Promise<T> {
    const now = Date.now();
    const existing = this.store.get(key) as CacheEntry<T> | undefined;

    if (existing) {
      if (existing.promise) {
        return existing.promise;
      }
      if (existing.expiresAt > now) {
        return Promise.resolve(existing.value);
      }
      this.store.delete(key);
    }

    const promise = fetcher().then((value) => {
      this.store.set(key, { value: value as unknown, expiresAt: now + ttlMs });
      return value;
    });

    this.store.set(key, { value: undefined as unknown as T, expiresAt: now + ttlMs, promise });

    // Evict oldest entries if over limit
    if (this.store.size > MAX_ENTRIES) {
      const oldest = this.store.keys().next().value;
      if (oldest !== undefined) {
        const entry = this.store.get(oldest);
        if (!entry?.promise) {
          this.store.delete(oldest);
        }
      }
    }

    return promise;
  }
}

export const apiCache = new ApiCache();
