import { useCallback, useRef } from "react";
import type { PositionMap } from "../lib/graphUtils";

interface CacheEntry {
  key: string;
  positions: PositionMap;
  timestamp: number;
}

const DB_NAME = "klynt-graph-positions";
const STORE_NAME = "positions";
const DB_VERSION = 1;
const MAX_ENTRIES = 15; // ~3 entries per 5 view modes

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: "key" });
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function readFromIDB(key: string): Promise<PositionMap | null> {
  try {
    const db = await openDB();
    return new Promise((resolve) => {
      const tx = db.transaction(STORE_NAME, "readonly");
      const store = tx.objectStore(STORE_NAME);
      const req = store.get(key);
      req.onsuccess = () => {
        const entry = req.result as CacheEntry | undefined;
        resolve(entry?.positions ?? null);
      };
      req.onerror = () => resolve(null);
    });
  } catch {
    return null;
  }
}

async function writeToIDB(key: string, positions: PositionMap): Promise<void> {
  try {
    const db = await openDB();
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    // Count first, then put + evict — all on same transaction to prevent auto-commit race
    const countReq = store.count();
    await new Promise<void>((resolve, reject) => {
      countReq.onsuccess = () => {
        store.put({ key, positions, timestamp: Date.now() } satisfies CacheEntry);
        const total = countReq.result + 1;
        if (total > MAX_ENTRIES) {
          const toDelete = total - MAX_ENTRIES;
          let deleted = 0;
          const cursor = store.openCursor();
          cursor.onsuccess = () => {
            const c = cursor.result;
            if (c && deleted < toDelete) {
              c.delete();
              deleted++;
              c.continue();
            } else {
              resolve();
            }
          };
          cursor.onerror = () => reject(cursor.error);
        } else {
          resolve();
        }
      };
      countReq.onerror = () => reject(countReq.error);
    });
  } catch {
    // Fallback: try localStorage for small graphs
    try {
      const lsKey = `graph-pos:${key}`;
      localStorage.setItem(lsKey, JSON.stringify(positions));
    } catch {
      // Silently fail — graph will just re-layout
    }
  }
}

function buildCacheKey(viewMode: string, fingerprint: string): string {
  return `${viewMode}-${fingerprint}`;
}

/**
 * Position cache hook. Provides load/save operations for graph node positions.
 * Primary storage: IndexedDB. Fallback: localStorage.
 */
export function useGraphPositionCache(viewMode: string, fingerprint: string) {
  const cacheKeyRef = useRef(buildCacheKey(viewMode, fingerprint));
  // Synchronous update on every render so callbacks always use latest key
  cacheKeyRef.current = buildCacheKey(viewMode, fingerprint);

  const loadPositions = useCallback(async (): Promise<PositionMap | null> => {
    const key = cacheKeyRef.current;
    const idbResult = await readFromIDB(key);
    if (idbResult) return idbResult;
    // Fallback: localStorage
    try {
      const stored = localStorage.getItem(`graph-pos:${key}`);
      if (stored) return JSON.parse(stored) as PositionMap;
    } catch {
      // ignore
    }
    return null;
  }, []);

  const savePositions = useCallback(async (positions: PositionMap): Promise<void> => {
    const key = cacheKeyRef.current;
    await writeToIDB(key, positions);
  }, []);

  const clearPositions = useCallback(async (): Promise<void> => {
    const key = cacheKeyRef.current;
    try {
      const db = await openDB();
      const tx = db.transaction(STORE_NAME, "readwrite");
      tx.objectStore(STORE_NAME).delete(key);
    } catch {
      // ignore
    }
    try {
      localStorage.removeItem(`graph-pos:${key}`);
    } catch {
      // ignore
    }
  }, []);

  return { loadPositions, savePositions, clearPositions };
}
