import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/api/client";

export type SkillListItem = {
  name: string;
  description: string;
  source: string;
  source_path: string;
  tags: string[];
  enabled: boolean;
};

export function SkillsSubsection() {
  const [skills, setSkills] = useState<SkillListItem[]>([]);
  const [installSrc, setInstallSrc] = useState("");

  const reload = useCallback(async () => {
    const list = (await invoke("coding_skills_list")) as SkillListItem[];
    setSkills(list);
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return (
    <section>
      <h3>Installed skills</h3>
      <ul>
        {skills.map((s) => (
          <li key={s.name}>
            <strong>{s.name}</strong> ({s.source}) — {s.description}
            <button
              type="button"
              onClick={async () => {
                await invoke("coding_skills_toggle", { name: s.name, enabled: !s.enabled });
                reload();
              }}
            >
              {s.enabled ? "Disable" : "Enable"}
            </button>
            {s.source === "user" && (
              <button
                type="button"
                onClick={async () => {
                  await invoke("coding_skills_uninstall", { name: s.name });
                  reload();
                }}
              >
                Uninstall
              </button>
            )}
          </li>
        ))}
      </ul>
      <h3>Install a new skill</h3>
      <input
        value={installSrc}
        onChange={(e) => setInstallSrc(e.target.value)}
        placeholder="Local path or git URL"
      />
      <button
        type="button"
        onClick={async () => {
          if (!installSrc) return;
          await invoke("coding_skills_install", { source: installSrc });
          setInstallSrc("");
          reload();
        }}
      >
        Install
      </button>
      <button type="button" onClick={() => invoke("coding_skills_reload").then(reload)}>
        Reload
      </button>
    </section>
  );
}
