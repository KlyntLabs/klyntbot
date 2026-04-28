import type { ProviderId } from "./types";

const PROVIDERS: { id: ProviderId | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "claudeCode", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "kimiCli", label: "Kimi" },
  { id: "openCode", label: "opencode" },
  { id: "klyntCli", label: "Klynt CLI" },
];

interface Props {
  active: ProviderId | "all";
  onChange: (id: ProviderId | "all") => void;
  counts?: Partial<Record<ProviderId | "all", number>>;
}

export function ProviderChips({ active, onChange, counts }: Props) {
  return (
    <div className="cm-provider-chips" role="tablist" aria-label="Coding CLI providers">
      {PROVIDERS.map((p) => (
        <button
          key={p.id}
          type="button"
          role="tab"
          aria-selected={active === p.id}
          className={"cm-provider-chip" + (active === p.id ? " cm-provider-chip--active" : "")}
          onClick={() => onChange(p.id)}
        >
          {p.label}
          {counts?.[p.id] != null && <span className="cm-provider-chip__count">{counts[p.id]}</span>}
        </button>
      ))}
    </div>
  );
}
