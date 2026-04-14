import type { InstalledSkill } from "@shared/types";
import type { ReactNode } from "react";

interface Props {
  sourceRef: string;
  installed?: InstalledSkill;
}

export function SkillDetailSidebar({ sourceRef, installed }: Props) {
  return (
    <aside className="w-60 flex-shrink-0 glass-panel border-l border-border p-4 space-y-4 text-sm">
      <Section label="Repository">
        <a
          href={`https://github.com/${sourceRef.split("/").slice(0, 2).join("/")}`}
          target="_blank"
          rel="noreferrer"
          className="text-brand hover:underline break-all"
        >
          {sourceRef}
        </a>
      </Section>
      {installed && (
        <>
          <Section label="Installed version">{installed.installedVersion}</Section>
          <Section label="Commit">{installed.installedSha.slice(0, 7)}</Section>
          {installed.bootstrappedDatabases.length > 0 && (
            <Section label="Databases">{installed.bootstrappedDatabases.length} managed</Section>
          )}
        </>
      )}
    </aside>
  );
}

function Section({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <p className="text-xs uppercase tracking-wide text-muted-foreground mb-1">{label}</p>
      <div className="text-foreground">{children}</div>
    </div>
  );
}
