import type { SkillBrowseRow } from "@shared/types";
import { Check, Package } from "lucide-react";
import { useNavigate } from "react-router";

interface Props {
  row: SkillBrowseRow;
}

export function SkillRow({ row }: Props) {
  const navigate = useNavigate();
  const encoded = encodeURIComponent(row.sourceRef);
  return (
    <button
      type="button"
      onClick={() => navigate(`/skills/${encoded}`)}
      className="w-full grid grid-cols-[48px_1fr_120px_120px] gap-4 items-center px-4 py-2 hover:bg-accent/20 border-b border-border text-left"
    >
      <span className="text-sm text-muted-foreground font-mono">
        {row.isInstalled ? <Check className="w-4 h-4 text-brand" /> : row.rank}
      </span>
      <span className="flex flex-col min-w-0">
        <span className="text-sm font-medium text-foreground truncate">{row.name}</span>
        <span className="text-xs text-muted-foreground truncate">{row.sourceRef}</span>
      </span>
      <span className="text-right text-sm text-muted-foreground">
        {row.installs !== undefined ? formatCount(row.installs) : "—"}
      </span>
      <span className="text-right">
        {row.isBundled ? (
          <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
            <Package className="w-3 h-3" /> Built-in
          </span>
        ) : row.isInstalled ? (
          <span className="text-xs text-brand">Installed</span>
        ) : row.isKlyntNative ? (
          <span className="text-xs text-accent">Klynt</span>
        ) : (
          <span className="text-xs text-muted-foreground">Prompt-only</span>
        )}
      </span>
    </button>
  );
}

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
