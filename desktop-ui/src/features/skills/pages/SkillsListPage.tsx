import { Search } from "lucide-react";
import { useState } from "react";
import { SkillRow } from "../components/SkillRow";
import { useSkillBrowse } from "../hooks/useSkillBrowse";
import { useSkillList } from "../hooks/useSkillList";

type Tab = "installed" | "all" | "trending" | "updates";

export function SkillsListPage() {
  const [tab, setTab] = useState<Tab>("all");
  const [query, setQuery] = useState("");
  const { data: browse } = useSkillBrowse(query || undefined);
  const { data: installed } = useSkillList();

  const rows = (() => {
    if (tab === "installed") {
      return (installed ?? []).map((s, i) => ({
        rank: i + 1,
        name: s.name,
        sourceRef: s.sourceRef,
        installs: undefined,
        isKlyntNative: !s.isAdapted,
        isInstalled: true,
        isBundled: s.sourceType === "bundled",
      }));
    }
    return browse ?? [];
  })();

  return (
    <div className="flex flex-col h-full">
      <header className="flex items-center justify-between px-6 py-4 border-b border-border">
        <h1 className="text-xl font-semibold text-foreground">Skills</h1>
      </header>
      <div className="px-6 py-3 border-b border-border">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search skills..."
            className="w-full pl-9 pr-3 py-2 bg-surface-base border border-border rounded-md text-sm text-foreground"
          />
        </div>
      </div>
      <nav className="flex gap-4 px-6 py-2 border-b border-border text-sm">
        {(["installed", "all", "trending", "updates"] as Tab[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={
              tab === t
                ? "text-foreground font-medium"
                : "text-muted-foreground hover:text-foreground"
            }
          >
            {labelFor(t, installed?.length)}
          </button>
        ))}
      </nav>
      <div className="grid grid-cols-[48px_1fr_120px_120px] gap-4 px-4 py-2 text-xs uppercase tracking-wide text-muted-foreground border-b border-border">
        <span>#</span>
        <span>Skill</span>
        <span className="text-right">Installs</span>
        <span className="text-right">Status</span>
      </div>
      <div className="flex-1 overflow-y-auto">
        {rows.map((r) => (
          <SkillRow key={r.sourceRef} row={r} />
        ))}
      </div>
    </div>
  );
}

function labelFor(t: Tab, installedCount: number | undefined): string {
  switch (t) {
    case "installed":
      return `Installed${installedCount != null ? ` (${installedCount})` : ""}`;
    case "all":
      return "All time";
    case "trending":
      return "Trending";
    case "updates":
      return "Updates";
  }
}
