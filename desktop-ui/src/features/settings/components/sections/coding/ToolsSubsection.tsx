import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

type CodingSection = { toolProfile?: "minimal" | "curated" | "power" };

export function ToolsSubsection() {
  const [profile, setProfile] = useState<"minimal" | "curated" | "power">("curated");

  useEffect(() => {
    (async () => {
      try {
        const cfg = (await invoke("config_get_section", { section: "coding" })) as CodingSection;
        setProfile(cfg.toolProfile ?? "curated");
      } catch (e) {
        console.warn("[ToolsSubsection] load failed", e);
      }
    })();
  }, []);

  const save = () =>
    invoke("config_update_section", {
      section: "coding",
      patch: { toolProfile: profile },
    });

  return (
    <section>
      <h3>Default tool profile</h3>
      <p>This applies to new threads. Use `/power on|off` to toggle per-thread.</p>
      {(["minimal", "curated", "power"] as const).map((p) => (
        <label key={p}>
          <input
            type="radio"
            name="profile"
            value={p}
            checked={profile === p}
            onChange={() => setProfile(p)}
          />
          {p}
        </label>
      ))}
      <button type="button" onClick={save}>
        Save
      </button>
    </section>
  );
}
