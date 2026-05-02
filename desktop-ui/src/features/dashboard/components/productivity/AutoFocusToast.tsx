import { useCallback, useEffect, useState } from "react";
import type { AutoFocusPayload, FocusSessionResponse } from "@/bindings";
import { productivityAutoFocusConfirm } from "@/api/endpoints/dashboard";
import { useTauriMutation } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { subscribeFocusAutoDetected } from "@/services/events";
import { AppIcon, getAppColor } from "../../lib/productivity";

export function AutoFocusToast() {
  const [session, setSession] = useState<AutoFocusPayload | null>(null);

  useEffect(() => {
    return subscribeFocusAutoDetected((payload) => {
      setSession(payload);
    });
  }, []);

  const { mutate: confirm, isLoading: confirming } = useTauriMutation<
    FocusSessionResponse,
    AutoFocusPayload
  >({
    mutationFn: productivityAutoFocusConfirm,
    invalidates: [qk.dashboard.all(), qk.productivity.all()],
  });

  const handleConfirm = useCallback(async () => {
    if (!session) return;
    try {
      await confirm(session);
    } finally {
      setSession(null);
    }
  }, [session, confirm]);

  const handleDismiss = useCallback(() => {
    setSession(null);
  }, []);

  if (!session) return null;

  const color = getAppColor(session.dominantApp, null);
  const ratio = Math.round(session.productiveRatio * 100);

  return (
    <div className="dashboard__auto-focus-toast">
      <div className="dashboard__auto-focus-toast-icon">
        <AppIcon appName={session.dominantApp} color={color} />
      </div>

      <div className="dashboard__auto-focus-toast-body">
        <div>
          <span>Focus session detected</span>
          <span className="dashboard__auto-focus-toast-ratio">{ratio}% productive</span>
        </div>
        <p>
          {session.durationMins}min in {session.dominantApp}
        </p>
      </div>

      <div className="dashboard__auto-focus-toast-actions">
        <button
          type="button"
          onClick={handleConfirm}
          disabled={confirming}
          className="dashboard__auto-focus-toast-confirm"
        >
          {confirming ? "Saving..." : "Confirm"}
        </button>
        <button
          type="button"
          onClick={handleDismiss}
          className="dashboard__auto-focus-toast-dismiss"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
