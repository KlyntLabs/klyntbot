import { useState } from "react";
import { GeneralSubsection } from "./coding/GeneralSubsection";
import { PermissionsSubsection } from "./coding/PermissionsSubsection";
import { SandboxSubsection } from "./coding/SandboxSubsection";
import { SessionsSubsection } from "./coding/SessionsSubsection";
import { SkillsSubsection } from "./coding/SkillsSubsection";
import { ToolsSubsection } from "./coding/ToolsSubsection";

const TABS = ["General", "Tools", "Permissions", "Sandbox", "Skills", "Sessions"] as const;
type Tab = (typeof TABS)[number];

export function SettingsCodingSection() {
  const [tab, setTab] = useState<Tab>("General");
  return (
    <div className="settings-coding">
      <nav className="settings-coding__tabs">
        {TABS.map((t) => (
          <button key={t} className={t === tab ? "active" : ""} onClick={() => setTab(t)}>
            {t}
          </button>
        ))}
      </nav>
      <div className="settings-coding__pane">
        {tab === "General" && <GeneralSubsection />}
        {tab === "Tools" && <ToolsSubsection />}
        {tab === "Permissions" && <PermissionsSubsection />}
        {tab === "Sandbox" && <SandboxSubsection />}
        {tab === "Skills" && <SkillsSubsection />}
        {tab === "Sessions" && <SessionsSubsection />}
      </div>
    </div>
  );
}
