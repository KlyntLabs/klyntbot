import { useEffect, useRef, useState } from "react";
import { invoke } from "@/api/client";
import { useEvent } from "@/hooks/useEvent";
import { useTransparentBackground } from "@/hooks/window/useTransparentBackground";
import { useWindowAutoResize } from "@/hooks/window/useWindowAutoResize";
import { useTauriMutation } from "@/lib/query";
import { getCurrentWindow, isTauri } from "@/utils/tauri-bridge";

import "../distraction.css";

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
  const bodyRef = useRef<HTMLDivElement>(null);

  useTransparentBackground();
  useWindowAutoResize(bodyRef, { width: 340, minHeight: 120, maxHeight: 300 });

  useEvent<InterventionPayload>("distraction:intervention", (payload) => {
    setIntervention(payload);
    setVerdict(null);
    if (payload.needsLlm) setLoading(true);
  });

  // Pull-on-mount: covers the cold-start race where the backend emitted
  // `distraction:intervention` before this listener was registered. The
  // backend caches the latest alert in a process-wide slot; we drain it
  // here and clear it once the user acts (see hideWindow below).
  useEffect(() => {
    let cancelled = false;
    void invoke<InterventionPayload | null>("distraction_get_pending_intervention")
      .then((payload) => {
        if (cancelled || !payload) return;
        setIntervention((prev) => prev ?? payload);
        if (payload.needsLlm) setLoading(true);
      })
      .catch(() => {
        // dev/browser without backend — ignore
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEvent<VerdictPayload>("distraction:verdict", (payload) => {
    setVerdict(payload);
    setLoading(false);
  });

  const dismiss = useTauriMutation<void, { appName: string }>({
    command: "distraction_dismiss",
    invalidates: [],
  });
  const allowTemp = useTauriMutation<void, { pattern: string }>({
    command: "distraction_allow_temp",
    invalidates: [],
  });
  const allowSession = useTauriMutation<
    void,
    { appName: string; windowTitle: string | null; classification: string }
  >({
    command: "distraction_allow_session",
    invalidates: [],
  });

  const titleExcerpt =
    intervention?.windowTitle && intervention.windowTitle.length > 50
      ? `${intervention.windowTitle.slice(0, 50)}…`
      : (intervention?.windowTitle ?? null);

  const hideWindow = async () => {
    setIntervention(null);
    setVerdict(null);
    setLoading(false);
    try {
      await invoke("distraction_clear_pending_intervention");
    } catch {
      // dev/browser without backend — ignore
    }
    if (!isTauri()) return;
    try {
      await getCurrentWindow().hide();
    } catch {
      // dev/browser mode — just clear state
    }
  };

  const pattern = intervention?.windowTitle?.toLowerCase() ?? intervention?.appName.toLowerCase();

  const handleDismiss = async () => {
    if (!intervention) return;
    await dismiss
      .mutate({ appName: intervention.appName })
      .catch((e) => console.error("Failed to dismiss distraction:", e));
    await hideWindow();
  };

  const handleAllowTemp = async () => {
    if (!pattern) return;
    await allowTemp.mutate({ pattern }).catch((e) => console.error("Failed to allow temp:", e));
    await hideWindow();
  };

  const handleAllowSession = async () => {
    if (!intervention) return;
    await allowSession
      .mutate({
        appName: intervention.appName,
        windowTitle: intervention.windowTitle,
        classification: verdict?.classification ?? "work_research",
      })
      .catch((e) => console.error("Failed to allow session:", e));
    await hideWindow();
  };

  const isPositiveVerdict =
    verdict?.classification === "educational" || verdict?.classification === "work_research";

  return (
    <div className="dx-root">
      <div ref={contentRef} className="dx-shell">
        <div ref={bodyRef} className="dx-body">
          {intervention && (
            <>
              <div className="dx-header">
                <div className="dx-status">
                  <span className="dx-status-dot" />
                  <span className="dx-status-label">Focus active</span>
                </div>
                {(loading || verdict) && (
                  <div className="dx-verdict">
                    {loading && (
                      <>
                        <div className="dx-dots">
                          <span style={{ animationDelay: "0ms" }} />
                          <span style={{ animationDelay: "150ms" }} />
                          <span style={{ animationDelay: "300ms" }} />
                        </div>
                        <span>Analyzing…</span>
                      </>
                    )}
                    {verdict && !loading && (
                      <span className={isPositiveVerdict ? "is-positive" : "is-negative"}>
                        {verdict.displayText}
                      </span>
                    )}
                  </div>
                )}
              </div>

              <div className="dx-divider" />

              <div className="dx-info">
                <div className="dx-app-name">{intervention.appName}</div>
                {titleExcerpt && <div className="dx-window-title">{titleExcerpt}</div>}
              </div>

              <div className="dx-actions">
                <button type="button" onClick={handleDismiss} className="dx-btn is-primary">
                  Back to work
                </button>
                <button type="button" onClick={handleAllowTemp} className="dx-btn">
                  5 min break
                </button>
                <button type="button" onClick={handleAllowSession} className="dx-btn">
                  It's work
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
