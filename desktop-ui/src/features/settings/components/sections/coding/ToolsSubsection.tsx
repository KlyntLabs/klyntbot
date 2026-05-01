import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function ToolsSubsection() {
  const [profile, setProfile] = useState<"minimal" | "curated" | "power">("curated");

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as {
        toolProfile?: "minimal" | "curated" | "power";
      };
      setProfile(cfg.toolProfile ?? "curated");
    })();
  }, []);

  return (
    <section>
      <h3>Default tool profile</h3>
      <p>This applies to new threads. Use `/power on|off` to toggle per-thread.</p>
      {(["minimal", "curated", "power"] as const).map((p) => (
        <label key={p}>
          <input type="radio" name="profile" value={p} checked={profile === p} onChange={() => setProfile(p)} />
          {p}
        </label>
      ))}
      <button type="button" onClick={() => invoke("config_set_coding_tools", { profile })}>
        Save
      </button>
    </section>
  );
}
