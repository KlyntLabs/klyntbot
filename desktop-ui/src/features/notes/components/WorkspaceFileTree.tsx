import { useQuery } from "@shared/hooks/useQuery";
import type { WorkspaceFile } from "@shared/types";
import { ChevronDown, ChevronRight, Settings } from "lucide-react";
import { memo, useCallback, useState } from "react";

interface WorkspaceFileTreeProps {
  activeFile: string | null;
  onSelectFile: (filename: string) => void;
}

const FILE_ICONS: Record<string, string> = {
  "SOUL.md": "\u{1F4AD}",
  "AGENTS.md": "\u{1F916}",
  "USER.md": "\u{1F464}",
  "TOOLS.md": "\u{1F527}",
  "RESPONSE.md": "\u{1F4AC}",
  "HEARTBEAT.md": "\u{1F493}",
};

export const WorkspaceFileTree = memo(function WorkspaceFileTree({
  activeFile,
  onSelectFile,
}: WorkspaceFileTreeProps) {
  const { data: files } = useQuery<WorkspaceFile[]>("workspace_list_files", undefined, []);

  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("workspace-config-collapsed") !== "false";
    } catch {
      return true;
    }
  });

  const toggleCollapsed = useCallback(() => {
    setCollapsed((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("workspace-config-collapsed", String(next));
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  if (files.length === 0) return null;

  return (
    <div className="mt-2 pt-2 border-t border-white/[0.06]">
      <button
        type="button"
        onClick={toggleCollapsed}
        className="w-full flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-muted hover:text-secondary transition-colors"
      >
        {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
        <Settings size={12} />
        <span>System Config</span>
      </button>

      {!collapsed && (
        <div className="mt-0.5">
          {files.map((file) => (
            <button
              key={file.name}
              type="button"
              onClick={() => onSelectFile(file.name)}
              title={file.description}
              className={`w-full text-left flex items-center gap-2 px-3 py-1 text-xs transition-colors ${
                activeFile === file.name
                  ? "bg-white/[0.08] text-primary"
                  : "text-muted hover:text-secondary hover:bg-white/[0.03]"
              }`}
            >
              <span className="text-[11px]">{FILE_ICONS[file.name] ?? "\u{1F4C4}"}</span>
              <span className="font-mono truncate">{file.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
});
