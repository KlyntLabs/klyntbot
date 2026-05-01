import { useEffect, useState } from "react";
import { invoke } from "@/api/client";

export function GeneralSubsection() {
  const [defaultMode, setDefaultMode] = useState<"general" | "coding">("general");
  const [autoDetect, setAutoDetect] = useState(true);

  useEffect(() => {
    (async () => {
      const cfg = (await invoke("config_get_coding")) as {
        defaultMode?: "general" | "coding";
        autoDetectFromWorkspace?: boolean;
      };
      setDefaultMode(cfg.defaultMode ?? "general");
      setAutoDetect(cfg.autoDetectFromWorkspace ?? true);
    })();
  }, []);

  const save = async () => {
    await invoke("config_set_coding_general", { defaultMode, autoDetect });
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
