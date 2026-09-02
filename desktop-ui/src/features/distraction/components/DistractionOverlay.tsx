import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import * as tauriWindow from "@tauri-apps/api/window";
import { useRef, useState } from "react";

interface InterventionPayload {
  appName: string;
  windowTitle: string | null;
  sessionId: string;
  needsLlm: boolean;
  heuristicVerdict: string;
}

interface VerdictPayload {
  classification: string;
  displayText: string;
}

export function DistractionOverlay() {
  const [intervention, setIntervention] = useState<InterventionPayload | null>(null);
  const [verdict, setVerdict] = useState<VerdictPayload | null>(null);
  const [loading, setLoading] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 340, maxHeight: 300 });

  useEvent<InterventionPayload>("distraction:intervention", (payload) => {
    setIntervention(payload);
    setVerdict(null);
    if (payload.needsLlm) {
      setLoading(true);
    }
  });

  useEvent<VerdictPayload>("distraction:verdict", (payload) => {
    setVerdict(payload);
    setLoading(false);
  });

  const titleExcerpt =
    intervention?.windowTitle && intervention.windowTitle.length > 50
      ? `${intervention.windowTitle.slice(0, 50)}\u2026`
      : (intervention?.windowTitle ?? null);

  const hideWindow = async () => {
    setIntervention(null);
    setVerdict(null);
    setLoading(false);
    try {
      await tauriWindow.getCurrentWindow().hide();
    } catch {
      // In dev browser mode, just clear state
    }
  };

  const pattern = intervention?.windowTitle?.toLowerCase() ?? intervention?.appName.toLowerCase();

  const handleDismiss = async () => {
    if (!intervention) return;
    await ipc("distraction_dismiss", {
      appName: intervention.appName,
    }).catch((e) => console.error("Failed to dismiss distraction:", e));
    await hideWindow();
  };

  const handleAllowTemp = async () => {
    if (!pattern) return;
    await ipc("distraction_allow_temp", { pattern }).catch((e) =>
      console.error("Failed to allow temp:", e),
    );
    await hideWindow();
  };

  const handleAllowSession = async () => {
    if (!intervention) return;
    await ipc("distraction_allow_session", {
      appName: intervention.appName,
      windowTitle: intervention.windowTitle,
      classification: verdict?.classification ?? "work_research",
    }).catch((e) => console.error("Failed to allow session:", e));
    await hideWindow();
  };

  // Always render the container so useWindowAutoResize observer stays attached.
  // Content is conditionally shown inside.
  return (
    <div
      ref={contentRef}
      className="w-full glass-floating overflow-hidden text-fg"
      style={{ animation: intervention ? "glass-appear 0.25s ease-out" : undefined }}
    >
      {intervention && (
        <div className="rounded-[calc(var(--ds-radius-card) - var(--ds-space-1-5))] overflow-hidden">
          <div className="px-4 py-3.5 flex flex-col gap-3">
            {/* Header */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span
                  className="w-1.5 h-1.5 rounded-full animate-pulse"
                  style={{ background: "var(--ds-status-danger)" }}
                />
                <span className="text-ui-xs text-status-danger font-medium uppercase tracking-wider">
                  Focus active
                </span>
              </div>
              {(loading || verdict) && (
                <div className="text-ui-xs text-fg-secondary flex items-center gap-1.5">
                  {loading && (
                    <>
                      <ThinkingDots size="sm" />
                      Analyzing...
                    </>
                  )}
                  {verdict && !loading && (
                    <span
                      className={
                        verdict.classification === "educational" ||
                        verdict.classification === "work_research"
                          ? "text-status-success"
                          : "text-status-danger"
                      }
                    >
                      {verdict.displayText}
                    </span>
                  )}
                </div>
              )}
            </div>

            {/* Content */}
            <div className="glass-divider" />
            <div className="flex flex-col gap-1">
              <div className="text-ui font-medium text-fg">{intervention.appName}</div>
              {titleExcerpt && (
                <div className="text-ui-xs font-light text-fg-secondary truncate">
                  {titleExcerpt}
                </div>
              )}
            </div>

            {/* Actions */}
            <div className="flex gap-2 pt-0.5">
              <button
                type="button"
                onClick={handleDismiss}
                className="flex-1 px-3 py-2 rounded-control text-ui-xs font-medium transition-all text-status-danger"
                style={{
                  background: "color-mix(in srgb, var(--ds-status-danger) 6%, transparent)",
                  border: "1px solid color-mix(in srgb, var(--ds-status-danger) 15%, transparent)",
                }}
              >
                Back to work
              </button>
              <button
                type="button"
                onClick={handleAllowTemp}
                className="flex-1 px-3 py-2 rounded-control text-ui-xs font-medium glass-button text-fg-secondary hover:text-fg transition-colors"
              >
                5 min break
              </button>
              <button
                type="button"
                onClick={handleAllowSession}
                className="flex-1 px-3 py-2 rounded-control text-ui-xs font-medium glass-button text-fg-secondary hover:text-fg transition-colors"
              >
                It's work
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
