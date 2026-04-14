import type { InstalledSkill, UninstallMode } from "@shared/types";
import { useState } from "react";
import { useSkillUninstall } from "../hooks/useSkillInstall";

interface Props {
  skill: InstalledSkill;
  onClose: () => void;
}

export function UninstallDialog({ skill, onClose }: Props) {
  const [mode, setMode] = useState<UninstallMode>("skill_only");
  const { mutate, loading } = useSkillUninstall();

  const handleUninstall = async () => {
    await mutate(skill.name, mode);
    onClose();
  };

  const hasDatabases = skill.bootstrappedDatabases.length > 0;

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: modal backdrop
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stop propagation only */}
      <div
        className="glass-panel rounded-lg p-6 max-w-md w-full"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-foreground mb-4">Uninstall {skill.name}?</h2>
        <div className="space-y-2 text-sm">
          <label className="flex items-start gap-2">
            <input
              type="radio"
              checked={mode === "skill_only"}
              onChange={() => setMode("skill_only")}
              className="mt-1"
            />
            <span>
              <span className="block text-foreground">Remove skill only</span>
              <span className="block text-xs text-muted-foreground">
                Databases stay. Safest choice.
              </span>
            </span>
          </label>
          {hasDatabases && (
            <>
              <label className="flex items-start gap-2">
                <input
                  type="radio"
                  checked={mode === "archive_databases"}
                  onChange={() => setMode("archive_databases")}
                  className="mt-1"
                />
                <span>
                  <span className="block text-foreground">Remove skill + archive databases</span>
                  <span className="block text-xs text-muted-foreground">
                    Renames {skill.bootstrappedDatabases.length} database(s) to "Archived: …".
                  </span>
                </span>
              </label>
              <label className="flex items-start gap-2">
                <input
                  type="radio"
                  checked={mode === "delete_databases"}
                  onChange={() => setMode("delete_databases")}
                  className="mt-1"
                />
                <span>
                  <span className="block text-red-400">Remove skill + delete data</span>
                  <span className="block text-xs text-muted-foreground">
                    Permanently deletes {skill.bootstrappedDatabases.length} database(s). Can't
                    undo.
                  </span>
                </span>
              </label>
            </>
          )}
        </div>
        <div className="flex justify-end gap-2 mt-6">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-muted-foreground"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={loading}
            onClick={handleUninstall}
            className="px-3 py-1.5 text-sm bg-red-500 text-white rounded"
          >
            {loading ? "Uninstalling..." : "Uninstall"}
          </button>
        </div>
      </div>
    </div>
  );
}
