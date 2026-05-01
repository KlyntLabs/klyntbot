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

type InstallStatus =
  | { kind: "idle" }
  | { kind: "installing" }
  | { kind: "ok"; text: string }
  | { kind: "err"; text: string };

function isValidSource(src: string): boolean {
  if (!src) return false;
  if (src.startsWith("http://") || src.startsWith("https://")) return true;
  if (src.startsWith("/") || src.startsWith(".")) return true;
  return false;
}

export function SkillsSubsection() {
  const [skills, setSkills] = useState<SkillListItem[]>([]);
  const [installSrc, setInstallSrc] = useState("");
  const [installStatus, setInstallStatus] = useState<InstallStatus>({ kind: "idle" });

  const reload = useCallback(async () => {
    const list = (await invoke("coding_skills_list")) as SkillListItem[];
    setSkills(list);
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const handleInstall = async () => {
    if (!installSrc) return;
    if (!isValidSource(installSrc)) {
      setInstallStatus({ kind: "err", text: "Enter a valid URL (http/https) or local path." });
      return;
    }
    setInstallStatus({ kind: "installing" });
    try {
      await invoke("coding_skills_install", { source: installSrc });
      setInstallSrc("");
      setInstallStatus({ kind: "ok", text: "Skill installed successfully." });
      reload();
    } catch (e) {
      setInstallStatus({ kind: "err", text: String(e) });
    }
  };

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
        onChange={(e) => {
          setInstallSrc(e.target.value);
          if (installStatus.kind !== "idle") setInstallStatus({ kind: "idle" });
        }}
        placeholder="Local path or git URL"
      />
      <button type="button" onClick={handleInstall} disabled={installStatus.kind === "installing"}>
        {installStatus.kind === "installing" ? "Installing…" : "Install"}
      </button>
      <button type="button" onClick={() => invoke("coding_skills_reload").then(reload)}>
        Reload
      </button>
      {installStatus.kind === "err" && (
        <p className="skills-subsection__feedback skills-subsection__feedback--err">
          {installStatus.text}
        </p>
      )}
      {installStatus.kind === "ok" && (
        <p className="skills-subsection__feedback skills-subsection__feedback--ok">
          {installStatus.text}
        </p>
      )}
    </section>
  );
}
