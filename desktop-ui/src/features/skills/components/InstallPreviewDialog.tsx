import type { InstallPlan } from "@shared/types";
import { useState } from "react";
import { useSkillInstallApply } from "../hooks/useSkillInstall";

interface Props {
  plan: InstallPlan;
  onClose: () => void;
  onInstalled?: () => void;
}

export function InstallPreviewDialog({ plan, onClose, onInstalled }: Props) {
  const [mode, setMode] = useState<"full" | "skillOnly">("full");
  const { mutate, loading } = useSkillInstallApply();

  const handleInstall = async () => {
    const effective = mode === "skillOnly" ? { ...plan, databasesToBootstrap: [] } : plan;
    const out = await mutate(effective);
    if (out) {
      onInstalled?.();
      onClose();
    }
  };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: modal backdrop
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
    >
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stop propagation only */}
      <div
        className="glass-panel rounded-lg p-6 max-w-lg w-full"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-foreground mb-4">Install {plan.package.name}</h2>
        <section className="mb-4">
          <h3 className="text-sm font-medium text-foreground mb-2">
            Files ({plan.filesToWrite.length})
          </h3>
          <ul className="text-xs text-muted-foreground space-y-1 max-h-32 overflow-y-auto">
            {plan.filesToWrite.map((f) => (
              <li key={f.relativePath} className="font-mono">
                {f.relativePath}{" "}
                <span className="text-muted-foreground/60">({f.contentSize}B)</span>
              </li>
            ))}
          </ul>
        </section>
        {plan.databasesToBootstrap.length > 0 && (
          <section className="mb-4">
            <h3 className="text-sm font-medium text-foreground mb-2">Databases to create</h3>
            <ul className="text-xs space-y-1">
              {plan.databasesToBootstrap.map((d) => (
                <li key={d.templateName} className="flex justify-between">
                  <span className="text-foreground">{d.databaseName}</span>
                  <span className="text-muted-foreground">{d.fieldCount} fields</span>
                </li>
              ))}
            </ul>
            <div className="mt-3 flex gap-2 text-sm">
              <label className="flex items-center gap-1">
                <input type="radio" checked={mode === "full"} onChange={() => setMode("full")} />{" "}
                Install + bootstrap
              </label>
              <label className="flex items-center gap-1">
                <input
                  type="radio"
                  checked={mode === "skillOnly"}
                  onChange={() => setMode("skillOnly")}
                />{" "}
                Install skill only
              </label>
            </div>
          </section>
        )}
        {plan.warnings.length > 0 && (
          <section className="mb-4">
            <h3 className="text-sm font-medium text-foreground mb-2">Warnings</h3>
            <ul className="text-xs text-amber-400 space-y-1">
              {plan.warnings.map((w, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: static warning list
                <li key={i}>{w}</li>
              ))}
            </ul>
          </section>
        )}
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
            onClick={handleInstall}
            className="px-3 py-1.5 text-sm bg-brand text-white rounded"
          >
            {loading ? "Installing..." : "Install"}
          </button>
        </div>
      </div>
    </div>
  );
}
