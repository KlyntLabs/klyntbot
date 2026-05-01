import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function SessionsSubsection() {
  const [retentionDays, setRetentionDays] = useState(90);
  const [maxDiskMb, setMaxDiskMb] = useState(5000);
  const [preserveStarred, setPreserveStarred] = useState(true);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as {
        sessions?: { retentionDays?: number; maxTotalDiskMb?: number; preserveStarred?: boolean };
      };
      setRetentionDays(cfg.sessions?.retentionDays ?? 90);
      setMaxDiskMb(cfg.sessions?.maxTotalDiskMb ?? 5000);
      setPreserveStarred(cfg.sessions?.preserveStarred ?? true);
    })();
  }, []);

  return (
    <section>
      <label>
        Retention (days):
        <input
          type="number"
          value={retentionDays}
          onChange={(e) => setRetentionDays(Number(e.target.value))}
        />
      </label>
      <label>
        Max total disk (MB):
        <input
          type="number"
          value={maxDiskMb}
          onChange={(e) => setMaxDiskMb(Number(e.target.value))}
        />
      </label>
      <label>
        <input
          type="checkbox"
          checked={preserveStarred}
          onChange={(e) => setPreserveStarred(e.target.checked)}
        />
        Preserve starred threads
      </label>
      <button
        type="button"
        onClick={() =>
          invoke("config_set_coding_sessions", { retentionDays, maxDiskMb, preserveStarred })
        }
      >
        Save
      </button>
    </section>
  );
}
