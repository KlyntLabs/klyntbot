import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

type CodingSection = {
  defaultMode?: "general" | "coding";
  autoDetectFromWorkspace?: boolean;
};

export function GeneralSubsection() {
  const [defaultMode, setDefaultMode] = useState<"general" | "coding">("general");
  const [autoDetect, setAutoDetect] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const cfg = (await invoke("config_get_section", { section: "coding" })) as CodingSection;
        setDefaultMode(cfg.defaultMode ?? "general");
        setAutoDetect(cfg.autoDetectFromWorkspace ?? true);
      } catch (e) {
        console.warn("[GeneralSubsection] load failed", e);
      }
    })();
  }, []);

  const save = async () => {
    await invoke("config_update_section", {
      section: "coding",
      patch: { defaultMode, autoDetectFromWorkspace: autoDetect },
    });
  };

  return (
    <section>
      <label>
        Default mode for new threads:
        <select
          value={defaultMode}
          onChange={(e) => setDefaultMode(e.target.value as "general" | "coding")}
        >
          <option value="general">General</option>
          <option value="coding">Coding</option>
        </select>
      </label>
      <label>
        <input
          type="checkbox"
          checked={autoDetect}
          onChange={(e) => setAutoDetect(e.target.checked)}
        />
        Auto-detect coding mode when thread is created from a workspace
      </label>
      <button type="button" onClick={save}>
        Save
      </button>
    </section>
  );
}
