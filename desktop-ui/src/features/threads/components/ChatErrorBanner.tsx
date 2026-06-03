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
          <span className="flex-1">
            Thread <code>{zombie.key}</code> appears stuck (no response).
          </span>
          <button
            type="button"
            className="px-2 py-0.5 rounded border border-current bg-transparent cursor-pointer text-inherit text-ui-xs"
            onClick={() => handleReset(zombie.key)}
          >
            Force Reset
          </button>
          <button
            type="button"
            className="bg-transparent border-none cursor-pointer text-inherit text-[length:var(--fs-md)] leading-none px-0.5 py-1.5 hover:opacity-70"
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
