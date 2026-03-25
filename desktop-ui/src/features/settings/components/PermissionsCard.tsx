import { ipc } from "@shared/hooks/useIpc";
import { Check, ExternalLink, ShieldAlert, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

interface PermissionRow {
  label: string;
  description: string;
  checkCmd: string;
  openCmd: string;
}

const permissions: PermissionRow[] = [
  {
    label: "Accessibility",
    description:
      "Required for reading window titles, detecting the frontmost app, and activity tracking. Needed for distraction detection and productivity features to work.",
    checkCmd: "permissions_check_accessibility",
    openCmd: "permissions_open_accessibility",
  },
];

export function PermissionsCard() {
  const [statuses, setStatuses] = useState<Record<string, boolean | null>>({});
  const recheckTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const checkAll = useCallback(async () => {
    const entries = await Promise.all(
      permissions.map(async (p) => {
        try {
          const granted = await ipc<boolean>(p.checkCmd);
          return [p.label, granted] as const;
        } catch {
          return [p.label, null] as const;
        }
      }),
    );
    setStatuses(Object.fromEntries(entries));
  }, []);

  useEffect(() => {
    checkAll();
    return () => clearTimeout(recheckTimer.current);
  }, [checkAll]);

  const handleOpen = async (p: PermissionRow) => {
    await ipc(p.openCmd).catch(() => {});
    clearTimeout(recheckTimer.current);
    recheckTimer.current = setTimeout(checkAll, 2000);
  };

  return (
    <div className="bg-card rounded-lg border border-border p-4">
      <h3 className="text-[13px] font-medium text-muted-foreground mb-1">macOS Permissions</h3>
      <p className="text-[11px] text-dim mb-4">
        These permissions are required for productivity tracking and smart distraction detection.
      </p>

      <div className="space-y-3">
        {permissions.map((p) => {
          const status = statuses[p.label];
          const granted = status === true;
          const denied = status === false;

          return (
            <div key={p.label} className="flex items-start gap-3 p-3 rounded-lg bg-accent">
              {/* Status icon */}
              <div className="flex-shrink-0 mt-0.5">
                {granted ? (
                  <div
                    className="size-5 rounded-full flex items-center justify-center"
                    style={{
                      background: "color-mix(in srgb, var(--success) 15%, transparent)",
                    }}
                  >
                    <Check
                      className="size-3"
                      style={{ color: "var(--success)" }}
                      strokeWidth={2.5}
                    />
                  </div>
                ) : denied ? (
                  <div
                    className="size-5 rounded-full flex items-center justify-center"
                    style={{
                      background: "color-mix(in srgb, var(--destructive) 15%, transparent)",
                    }}
                  >
                    <X
                      className="size-3"
                      style={{ color: "var(--destructive)" }}
                      strokeWidth={2.5}
                    />
                  </div>
                ) : (
                  <div className="size-5 rounded-full bg-muted" />
                )}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-[13px] font-medium text-foreground">{p.label}</span>
                  {granted && <span className="text-2xs text-success">Granted</span>}
                  {denied && (
                    <span className="text-2xs" style={{ color: "var(--destructive)" }}>
                      Not granted
                    </span>
                  )}
                </div>
                <p className="text-[11px] text-dim mt-0.5 leading-relaxed">{p.description}</p>
              </div>

              {/* Action */}
              {denied && (
                <button
                  type="button"
                  onClick={() => handleOpen(p)}
                  className="flex-shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors bg-brand/10 text-brand hover:bg-brand/20"
                >
                  <ExternalLink className="size-3" />
                  Open Settings
                </button>
              )}
              {granted && (
                <button
                  type="button"
                  onClick={() => handleOpen(p)}
                  className="flex-shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs text-dim hover:text-muted-foreground transition-colors"
                >
                  <ExternalLink className="size-3" />
                  Open
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Warning if any permission is denied */}
      {Object.values(statuses).some((s) => s === false) && (
        <div
          className="mt-3 flex items-start gap-2 p-3 rounded-lg text-xs"
          style={{
            background: "color-mix(in srgb, var(--destructive) 5%, transparent)",
            border: "1px solid color-mix(in srgb, var(--destructive) 15%, transparent)",
          }}
        >
          <ShieldAlert
            className="size-4 flex-shrink-0 mt-0.5"
            style={{ color: "var(--destructive)" }}
            strokeWidth={1.5}
          />
          <div>
            <span className="font-medium" style={{ color: "var(--destructive)" }}>
              Permissions needed.
            </span>{" "}
            <span className="text-muted-foreground">
              After granting permissions in System Settings, you may need to restart Klynt for
              changes to take effect.
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
