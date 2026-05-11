import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type ZombieSession = {
  key: string;
  mode: string;
  updatedAt: number;
};

/**
 * Global chat error / zombie session banner.
 *
 * On mount, queries the backend for zombie sessions (sessions where the
 * agent never replied). Surfaces a dismissible banner per zombie thread
 * with a "Force Reset" action.
 */
export function ChatErrorBanner() {
  const [zombies, setZombies] = useState<ZombieSession[]>([]);
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;
    invoke<ZombieSession[]>("chat_zombie_check", { thresholdMs: 5 * 60 * 1000 })
      .then((rows) => {
        if (cancelled) return;
        setZombies(rows);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const handleReset = useCallback(async (sessionKey: string) => {
    try {
      await invoke("chat_force_reset", { sessionKey });
      setZombies((prev) => prev.filter((z) => z.key !== sessionKey));
    } catch {
      // Ignore — user can retry
    }
  }, []);

  const visible = zombies.filter((z) => !dismissed.has(z.key));
  if (visible.length === 0) return null;

  return (
    <div className="chat-error-banner-stack" role="region" aria-label="Chat errors">
      {visible.map((zombie) => (
        <div key={zombie.key} className="chat-error-banner" role="alert">
          <span className="chat-error-banner__msg">
            Thread <code>{zombie.key}</code> appears stuck (no response).
          </span>
          <button
            type="button"
            className="chat-error-banner__btn"
            onClick={() => handleReset(zombie.key)}
          >
            Force Reset
          </button>
          <button
            type="button"
            className="chat-error-banner__dismiss"
            onClick={() =>
              setDismissed((prev) => new Set([...prev, zombie.key]))
            }
            aria-label="Dismiss"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
