import { useMemo, useState } from "react";
import { invoke } from "@/api/client";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { SessionSummary } from "./types";
import { useTracingSessions } from "./useTracingSessions";
import { SessionsToolbar, type SortKey } from "./SessionsToolbar";
import { SessionCard } from "./SessionCard";
import { ProjectGroup } from "./ProjectGroup";

interface Props {
  providerId: string;
  onOpenSession: (sessionId: string) => void;
}

export function SessionsListView({ providerId, onOpenSession }: Props) {
  const [refreshKey, setRefreshKey] = useState(0);
  const { sessions, loading, error } = useTracingSessions(providerId, refreshKey);
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortKey>("recent");
  const [importedOnly, setImportedOnly] = useState(false);
  const [group, setGroup] = useState(true);
  const [layout, setLayout] = useState<"grid" | "list">("grid");

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return sessions.filter((s) => {
      if (importedOnly && !s.imported) return false;
      if (!q) return true;
      return (
        s.sessionId.toLowerCase().includes(q) ||
        (s.customTitle ?? "").toLowerCase().includes(q) ||
        (s.cwd ?? "").toLowerCase().includes(q)
      );
    });
  }, [sessions, search, importedOnly]);

  const sorted = useMemo(() => {
    const a = [...filtered];
    a.sort((x, y) => {
      switch (sort) {
        case "recent": return y.lastEventAt.localeCompare(x.lastEventAt);
        case "created": return y.startedAt.localeCompare(x.startedAt);
        case "size": return y.sizeBytes - x.sizeBytes;
        case "errors": return y.errorCount - x.errorCount;
      }
    });
    return a;
  }, [filtered, sort]);

  const grouped = useMemo(() => {
    const map = new Map<string, SessionSummary[]>();
    for (const s of sorted) {
      const key = s.projectBasename ?? "(unknown)";
      const list = map.get(key) ?? [];
      list.push(s);
      map.set(key, list);
    }
    return Array.from(map.entries());
  }, [sorted]);

  const totalSize = sorted.reduce((acc, s) => acc + s.sizeBytes, 0);
  const projectCount = grouped.length;

  const handleImport = async () => {
    const file = await openDialog({
      multiple: false,
      filters: [{ name: "wire.jsonl", extensions: ["jsonl"] }],
    });
    if (typeof file === "string") {
      await invoke("tracing_import", { providerId, filePath: file });
      setRefreshKey((k) => k + 1);
    }
  };

  if (loading) return <div className="tracing-state">Loading sessions…</div>;
  if (error) return <div className="tracing-state tracing-state--error">{error}</div>;

  return (
    <div className="tracing-list">
      <SessionsToolbar
        sessionCount={sorted.length}
        search={search} onSearchChange={setSearch}
        sort={sort} onSortChange={setSort}
        importedOnly={importedOnly} onImportedToggle={() => setImportedOnly((v) => !v)}
        group={group} onGroupToggle={() => setGroup((v) => !v)}
        layout={layout} onLayoutToggle={() => setLayout((l) => (l === "grid" ? "list" : "grid"))}
        onImport={handleImport}
      />
      <div className="tracing-list__body">
        {group ? (
          grouped.map(([basename, items]) => (
            <ProjectGroup
              key={basename}
              basename={basename}
              cwd={items[0]?.cwd ?? ""}
              count={items.length}
              layout={layout}
            >
              {items.map((s) => (
                <SessionCard key={s.sessionId} summary={s} onClick={() => onOpenSession(s.sessionId)} />
              ))}
            </ProjectGroup>
          ))
        ) : (
          <div className={layout === "list" ? "tracing-list__flat tracing-list__flat--list" : "tracing-list__flat"}>
            {sorted.map((s) => (
              <SessionCard key={s.sessionId} summary={s} onClick={() => onOpenSession(s.sessionId)} />
            ))}
          </div>
        )}
      </div>
      <div className="tracing-list__footer">
        {sorted.length} sessions · {projectCount} projects · {(totalSize / (1024 * 1024)).toFixed(1)} MB total
      </div>
    </div>
  );
}
