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
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-1.5">
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
              className={`relative z-10 mt-1.5 size-2 rounded-full shrink-0 ${v.reverted ? "bg-muted-foreground" : "bg-accent"}`}
              style={{ marginLeft: "7px" }}
            />

            <div className="glass-card rounded-xl p-3 flex-1">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-[12px] font-medium text-foreground">
                    Version {v.version}
                  </span>
                  {v.reverted && (
                    <span className="text-2xs text-muted-foreground ml-2">(reverted)</span>
                  )}
                </div>
                <span className="text-2xs text-dim">
                  {new Date(v.promotedAt).toLocaleDateString()}
                </span>
              </div>
              <p className="text-[11px] text-muted-foreground mt-0.5">{v.reason}</p>

              {!v.reverted &&
                v.version !== versions[0]?.version &&
                (confirmVersion === v.version ? (
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-2xs text-muted-foreground">Revert to this version?</span>
                    <button
                      type="button"
                      onClick={() => handleRevert(v.version)}
                      disabled={loading}
                      className="text-2xs text-accent hover:text-accent/80 transition-colors"
                    >
                      Confirm
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmVersion(null)}
                      className="text-2xs text-muted-foreground"
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConfirmVersion(v.version)}
                    className="flex items-center gap-1 mt-2 text-2xs text-muted-foreground hover:text-accent transition-colors"
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
