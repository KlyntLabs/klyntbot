import { useEffect, useState } from "react";
import { readAgentMd } from "@/api/endpoints/files";

export type LoadedContextFile = {
  label: string;
  path: string;
};

/// Detects auto-loaded context files for a workspace (currently only
/// `AGENTS.md` at the workspace root). Returns an empty list when nothing is
/// detected so callers can hide the section. Existence is checked by reading
/// the file via the `workspace_meta_read` IPC, which already returns
/// `{ exists, ... }` — no extra `fs.exists` plumbing needed.
export function useLoadedContextFiles(
  workspaceId: string | null,
  workspacePath: string | null,
): LoadedContextFile[] {
  const [files, setFiles] = useState<LoadedContextFile[]>([]);

  useEffect(() => {
    if (!workspaceId || !workspacePath) {
      setFiles([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const result = await readAgentMd(workspaceId);
        if (cancelled) return;
        if (result.exists) {
          setFiles([{ label: "AGENTS.md", path: `${workspacePath}/AGENTS.md` }]);
        } else {
          setFiles([]);
        }
      } catch {
        if (!cancelled) setFiles([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [workspaceId, workspacePath]);

  return files;
}
