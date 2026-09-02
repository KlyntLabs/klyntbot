import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import { GitBranch, RotateCcw } from "lucide-react";
import { useState } from "react";

export interface BrainVersion {
  version: number;
  trialId: string | null;
  promotedAt: string;
  params: Record<string, unknown>;
  reason: string;
  parentVersion: number | null;
  metricsAtPromotion: Record<string, unknown>;
  reverted: boolean;
}

export function BrainTimeline() {
  const { data: versions } = useQuery<BrainVersion[]>("get_brain_versions", undefined, []);
  const { mutate: revert, loading } = useMutation<BrainVersion, { version: number }>(
    "revert_brain_version",
  );
  const [confirmVersion, setConfirmVersion] = useState<number | null>(null);

  if (!versions || versions.length === 0) return null;

  const handleRevert = async (version: number) => {
    await revert({ version });
    setConfirmVersion(null);
    invalidateQueries("get_brain_versions");
  };

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-ui font-medium text-fg-secondary flex items-center gap-1.5">
        <GitBranch className="size-3.5" />
        Brain Versions
      </h2>

      <div className="relative">
        <div className="absolute left-[11px] top-0 bottom-0 w-px bg-border-subtle" />

        {versions.map((v) => (
          <div
            key={v.version}
            className={`relative flex gap-3 pb-4 ${v.reverted ? "opacity-40" : ""}`}
          >
            <div
              className={`relative z-10 mt-1.5 size-2 rounded-full shrink-0 ${v.reverted ? "bg-fg-secondary" : "bg-brand"}`}
              style={{ marginLeft: "7px" }}
            />

            <div className="glass-card rounded-xl p-3 flex-1">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-ui-sm font-medium text-fg">
                    Version {v.version}
                  </span>
                  {v.reverted && (
                    <span className="text-ui-xs text-fg-secondary ml-2">(reverted)</span>
                  )}
                </div>
                <span className="text-ui-xs text-fg-dim">
                  {new Date(v.promotedAt).toLocaleDateString()}
                </span>
              </div>
              <p className="text-ui-xs text-fg-secondary mt-0.5">{v.reason}</p>

              {!v.reverted &&
                v.version !== versions[0]?.version &&
                (confirmVersion === v.version ? (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-ui-xs text-fg-secondary">Revert to this version?</span>
                    <button
                      type="button"
                      onClick={() => handleRevert(v.version)}
                      disabled={loading}
                      className="text-ui-xs text-brand hover:text-brand/80 transition-colors"
                    >
                      Confirm
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmVersion(null)}
                      className="text-ui-xs text-fg-secondary"
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmVersion(v.version)}
                    className="flex items-center gap-1 mt-2 text-ui-xs text-fg-secondary hover:text-brand transition-colors"
                  >
                    <RotateCcw className="size-3" />
                    Revert to this version
                  </button>
                ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
