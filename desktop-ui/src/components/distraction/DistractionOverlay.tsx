import * as tauriWindow from "@tauri-apps/api/window";
import { useState } from "react";
import { useEvent } from "../../hooks/useEvent";
import { ipc } from "../../hooks/useIpc";

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

  if (!intervention) {
    return (
      <div className="w-screen h-screen flex items-center justify-center">
        <div className="text-dim text-sm">Waiting...</div>
      </div>
    );
  }

  const titleExcerpt = intervention.windowTitle
    ? intervention.windowTitle.length > 60
      ? `${intervention.windowTitle.slice(0, 60)}...`
      : intervention.windowTitle
    : null;

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

  const pattern = intervention.windowTitle?.toLowerCase() ?? intervention.appName.toLowerCase();

  const handleDismiss = async () => {
    await ipc("distraction_dismiss", {
      appName: intervention.appName,
    }).catch(() => {});
    await hideWindow();
  };

  const handleAllowTemp = async () => {
    await ipc("distraction_allow_temp", { pattern }).catch(() => {});
    await hideWindow();
  };

  const handleAllowSession = async () => {
    await ipc("distraction_allow_session", {
      appName: intervention.appName,
      windowTitle: intervention.windowTitle,
      classification: verdict?.classification ?? "work_research",
    }).catch(() => {});
    await hideWindow();
  };

  return (
    <div className="w-screen h-screen flex items-center justify-center p-4">
      <div className="glass-panel w-full max-w-[400px] rounded-2xl overflow-hidden">
        {/* Red accent top bar */}
        <div className="h-[3px]" style={{ background: "var(--destructive)" }} />

        <div className="p-5 flex flex-col gap-4">
          {/* Header */}
          <div className="flex items-center gap-2">
            <span
              className="w-2 h-2 rounded-full animate-pulse"
              style={{ background: "var(--destructive)" }}
            />
            <span className="text-[13px] font-semibold" style={{ color: "var(--destructive)" }}>
              Focus Session Active
            </span>
          </div>

          {/* App info */}
          <div>
            <div className="text-[15px] font-medium text-primary">
              You switched to {intervention.appName}
            </div>
            {titleExcerpt && (
              <div className="text-[12px] text-muted mt-1 truncate">
                &ldquo;{titleExcerpt}&rdquo;
              </div>
            )}
          </div>

          {/* LLM verdict */}
          {(loading || verdict) && (
            <div className="text-[12px] text-dim flex items-center gap-2">
              {loading && (
                <>
                  <span className="w-3 h-3 border border-dim border-t-transparent rounded-full animate-spin" />
                  Analyzing content...
                </>
              )}
              {verdict && !loading && (
                <span
                  className={
                    verdict.classification === "educational" ||
                    verdict.classification === "work_research"
                      ? "text-success"
                      : "text-destructive"
                  }
                >
                  {verdict.displayText}
                </span>
              )}
            </div>
          )}

          {/* Action buttons */}
          <div className="flex gap-2 mt-1">
            <button
              type="button"
              onClick={handleDismiss}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium transition-colors"
              style={{
                background: "color-mix(in srgb, var(--destructive) 10%, transparent)",
                color: "var(--destructive)",
              }}
            >
              Back to work
            </button>
            <button
              type="button"
              onClick={handleAllowTemp}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium bg-surface-raised text-muted hover:text-primary hover:bg-surface-highest transition-colors"
            >
              Allow briefly
            </button>
            <button
              type="button"
              onClick={handleAllowSession}
              className="flex-1 px-3 py-2 rounded-lg text-[12px] font-medium bg-surface-raised text-muted hover:text-primary hover:bg-surface-highest transition-colors"
            >
              This is work
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
