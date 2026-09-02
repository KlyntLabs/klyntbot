import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";

interface EntityReference {
  type: string;
  id: string;
}

interface EntityReferencesPanelProps {
  noteBody: string | null;
  collapsed?: boolean;
}

/** Extract @type:id patterns from note body text */
function extractEntityMentions(body: string): EntityReference[] {
  const pattern = /@(task|project|area|goal|okr):([a-zA-Z0-9_-]+)/g;
  const refs: EntityReference[] = [];
  const seen = new Set<string>();
  let match: RegExpExecArray | null = null;

  while (true) {
    match = pattern.exec(body);
    if (!match) break;
    const key = `${match[1]}:${match[2]}`;
    if (!seen.has(key)) {
      seen.add(key);
      refs.push({ type: match[1], id: match[2] });
    }
  }

  return refs;
}

const TYPE_COLORS: Record<string, string> = {
  task: "rgba(96, 165, 250, 0.85)",
  project: "rgba(167, 139, 250, 0.85)",
  area: "rgba(52, 211, 153, 0.7)",
  goal: "rgba(251, 146, 60, 0.7)",
  okr: "rgba(248, 113, 113, 0.6)",
};

const TYPE_BG: Record<string, string> = {
  task: "rgba(96, 165, 250, 0.12)",
  project: "rgba(167, 139, 250, 0.12)",
  area: "rgba(52, 211, 153, 0.1)",
  goal: "rgba(251, 146, 60, 0.1)",
  okr: "rgba(248, 113, 113, 0.1)",
};

export function EntityReferencesPanel({
  noteBody,
  collapsed: initialCollapsed,
}: EntityReferencesPanelProps) {
  const [collapsed, setCollapsed] = useState(initialCollapsed ?? false);

  const entities = useMemo(() => {
    if (!noteBody) return [];
    return extractEntityMentions(noteBody);
  }, [noteBody]);

  return (
    <div className="border-b border-separator">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-3 py-2 text-ui-xs font-medium uppercase tracking-wider text-fg-secondary hover:text-fg transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <span>Entity References</span>
      </button>

      {!collapsed && (
        <div className="px-3 pb-2.5">
          {entities.length === 0 ? (
            <div className="text-ui-xs text-fg-dim py-1">No entity references</div>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {entities.map((ent) => (
                <span
                  key={`${ent.type}:${ent.id}`}
                  className="inline-flex items-center gap-1 text-ui-xs px-1.5 py-0.5 rounded-md cursor-default"
                  style={{
                    color: TYPE_COLORS[ent.type] ?? "rgba(255,255,255,0.6)",
                    backgroundColor: TYPE_BG[ent.type] ?? "rgba(255,255,255,0.06)",
                  }}
                >
                  <span className="font-medium">@{ent.type}</span>
                  <span className="opacity-70">{ent.id.slice(0, 8)}</span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
