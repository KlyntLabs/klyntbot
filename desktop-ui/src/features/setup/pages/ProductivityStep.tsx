import { useEffect, useId, useState } from "react";
import { useOutletContext } from "react-router";
import { ipc } from "@shared/hooks/useIpc";
import { Toggle } from "@shared/ui";
import type { SetupContext } from "../hooks/steps";

export function ProductivityStep() {
  const { forwardRef, setDirty } = useOutletContext<SetupContext>();
  const id = useId();

  const [enabled, setEnabled] = useState(true);
  const [focusDuration, setFocusDuration] = useState(45);
  const [dailyTarget, setDailyTarget] = useState(8);
  const [excludedApps, setExcludedApps] = useState("");

  // Load saved config on mount
  useEffect(() => {
    ipc<{
      enabled?: boolean;
      focus?: { defaultDurationMins?: number; maxDailyFocusHours?: number };
      privacy?: { excludedApps?: string[] };
    }>("config_get_section", { section: "productivity" })
      .then((saved) => {
        if (!saved || typeof saved !== "object") return;
        let hasSaved = false;
        if (saved.enabled !== undefined) {
          setEnabled(saved.enabled);
          hasSaved = true;
        }
        if (saved.focus?.defaultDurationMins) {
          setFocusDuration(saved.focus.defaultDurationMins);
          hasSaved = true;
        }
        if (saved.focus?.maxDailyFocusHours) {
          setDailyTarget(saved.focus.maxDailyFocusHours);
          hasSaved = true;
        }
        if (saved.privacy?.excludedApps?.length) {
          setExcludedApps(saved.privacy.excludedApps.join(", "));
          hasSaved = true;
        }
        if (hasSaved) setDirty(true);
      })
      .catch(() => {});
  }, [setDirty]);

  // Register save handler with layout
  useEffect(() => {
    forwardRef.current = async (isSkip: boolean) => {
      if (isSkip) return true;
      await ipc("config_update_section", {
        section: "productivity",
        patch: {
          enabled,
          focus: {
            defaultDurationMins: focusDuration,
            maxDailyFocusHours: dailyTarget,
          },
          privacy: {
            excludedApps: excludedApps
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean),
          },
        },
      });
      return true;
    };
  }, [forwardRef, enabled, focusDuration, dailyTarget, excludedApps]);

  return (
    <div>
      <h2 className="text-lg font-medium text-primary mb-1">Productivity</h2>
      <p className="text-[13px] text-muted mb-6">
        Configure focus sessions and activity tracking preferences.
      </p>

      <div className="space-y-5">
        <div className="flex items-center justify-between">
          <div>
            <span className="text-[13px] font-medium text-secondary">Enable tracking</span>
            <p className="text-[11px] text-dim mt-0.5">
              Track app usage and focus sessions automatically
            </p>
          </div>
          <Toggle
            checked={enabled}
            onChange={(v) => {
              setEnabled(v);
              setDirty(true);
            }}
          />
        </div>

        {enabled && (
          <>
            <label className="block">
              <span className="block text-[12px] font-medium text-secondary mb-1.5">
                Default focus session (minutes)
              </span>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={15}
                  max={120}
                  step={5}
                  value={focusDuration}
                  onChange={(e) => {
                    setFocusDuration(Number(e.target.value));
                    setDirty(true);
                  }}
                  className="flex-1 accent-brand"
                />
                <span className="text-[13px] text-secondary font-mono w-10 text-right">
                  {focusDuration}
                </span>
              </div>
            </label>

            <label className="block">
              <span className="block text-[12px] font-medium text-secondary mb-1.5">
                Daily focus target (hours)
              </span>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={16}
                  step={1}
                  value={dailyTarget}
                  onChange={(e) => {
                    setDailyTarget(Number(e.target.value));
                    setDirty(true);
                  }}
                  className="flex-1 accent-brand"
                />
                <span className="text-[13px] text-secondary font-mono w-10 text-right">
                  {dailyTarget}h
                </span>
              </div>
            </label>

            <label className="block">
              <span className="block text-[12px] font-medium text-secondary mb-1.5">
                Excluded apps (privacy)
              </span>
              <input
                id={`${id}-excluded`}
                type="text"
                value={excludedApps}
                onChange={(e) => {
                  setExcludedApps(e.target.value);
                  setDirty(true);
                }}
                placeholder="e.g. 1Password, Signal, Messages"
                className="w-full px-3 py-2 text-[13px] text-primary bg-surface-base border border-border rounded-lg focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim"
              />
              <p className="text-[11px] text-dim mt-1">
                Comma-separated list of apps to exclude from tracking
              </p>
            </label>
          </>
        )}
      </div>
    </div>
  );
}
