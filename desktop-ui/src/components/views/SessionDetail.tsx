import { MessageSquarePlus } from "lucide-react";
import { useNavigate, useParams } from "react-router";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import {
  type BrainstormConversation,
  sessionStatusColor,
  type TrackedSession,
} from "../../lib/session-types";
import { SessionMirrorView } from "../sessions/SessionMirrorView";
import { SessionSidebar } from "../sessions/SessionSidebar";

export function SessionDetail() {
  const { sessionId } = useParams<{ sessionId: string }>();
  const navigate = useNavigate();

  const { data: sessions } = useQuery<TrackedSession[]>("get_tracked_sessions", undefined, []);

  const createBrainstorm = useMutation<BrainstormConversation, { sessionId: string; mode: string }>(
    "create_brainstorm",
  );

  const session = sessions.find((s) => s.sessionId === sessionId);

  const handleNewBrainstorm = async () => {
    if (!sessionId) return;
    const conversation = await createBrainstorm.mutate({
      sessionId,
      mode: "directModel",
    });
    if (conversation) {
      navigate(`/sessions/${sessionId}/brainstorm/${conversation.id}`);
    }
  };

  if (!sessionId) return null;

  return (
    <div className="flex-1 flex overflow-hidden">
      <SessionSidebar activeSessionId={sessionId} />
      <div className="flex-1 flex flex-col overflow-hidden relative">
        {/* Header */}
        <div className="h-12 flex items-center px-6 gap-3 shrink-0">
          <span className="text-[13px] font-light text-secondary">
            {session?.projectName ?? "Session"}
          </span>
          {session?.gitBranch && (
            <span className="text-[11px] text-dim font-light">{session.gitBranch}</span>
          )}
          {session && (
            <span className={`text-[11px] font-light ${sessionStatusColor(session.status)}`}>
              {session.status}
            </span>
          )}
        </div>

        <div className="mx-6 glass-divider" />

        <SessionMirrorView sessionId={sessionId} showPinButtons />

        {/* Floating brainstorm button */}
        <button
          type="button"
          onClick={handleNewBrainstorm}
          disabled={createBrainstorm.loading}
          title="New brainstorm"
          className="absolute bottom-6 right-6 w-12 h-12 rounded-2xl bg-brand hover:bg-brand-hover disabled:opacity-50 flex items-center justify-center transition-all shadow-lg"
        >
          <MessageSquarePlus className="w-5 h-5 text-white" strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}
