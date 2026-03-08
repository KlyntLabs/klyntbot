import { useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import type { TrackedSession } from "../../lib/session-types";
import { BrainstormPanel } from "../sessions/BrainstormPanel";
import { SessionMirrorView } from "../sessions/SessionMirrorView";

export function BrainstormChat() {
  const { sessionId, brainstormId } = useParams<{
    sessionId: string;
    brainstormId: string;
  }>();

  const { data: sessions } = useQuery<TrackedSession[]>("get_tracked_sessions", undefined, []);

  const session = sessions.find((s) => s.sessionId === sessionId);

  if (!sessionId || !brainstormId || !session) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <span className="text-muted text-[13px] font-light">Loading session...</span>
      </div>
    );
  }

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Session mirror (left) */}
      <div className="flex-1 flex flex-col overflow-hidden border-r border-white/[0.06]">
        <div className="h-10 flex items-center px-5 shrink-0">
          <span className="text-[12px] font-light text-muted">Session Mirror</span>
        </div>
        <SessionMirrorView sessionId={sessionId} showPinButtons />
      </div>

      {/* Brainstorm panel (right) */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="h-10 flex items-center px-5 shrink-0">
          <span className="text-[12px] font-light text-muted">Brainstorm</span>
        </div>
        <BrainstormPanel conversationId={brainstormId} sessionId={sessionId} session={session} />
      </div>
    </div>
  );
}
