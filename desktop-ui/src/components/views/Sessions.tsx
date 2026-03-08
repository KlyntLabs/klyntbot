import { useEffect, useRef } from "react";
import { useNavigate } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import type { TrackedSession } from "../../lib/session-types";
import { SessionSidebar } from "../sessions/SessionSidebar";

export function Sessions() {
  const navigate = useNavigate();
  const { data: sessions } = useQuery<TrackedSession[]>("get_tracked_sessions", undefined, []);

  // Auto-redirect to first active session (once)
  const hasRedirectedRef = useRef(false);
  useEffect(() => {
    if (sessions.length === 0 || hasRedirectedRef.current) return;
    hasRedirectedRef.current = true;
    const active = sessions.find((s) => s.status === "active");
    const target = active ?? sessions[0];
    navigate(`/sessions/${target.sessionId}`, { replace: true });
  }, [sessions, navigate]);

  return (
    <div className="flex-1 flex overflow-hidden">
      <SessionSidebar />
      <div className="flex-1 flex items-center justify-center">
        <span className="text-muted text-[13px] font-light">Select a session to view</span>
      </div>
    </div>
  );
}
