import { useEffect, useRef } from "react";
import { useThreadEvents } from "../hooks/useThreadEvents";
import { ThreadItemList } from "./ThreadItemList";

type Props = {
  threadId: string | null;
  draftPrompt?: string | null;
  onDraftConsumed?: () => void;
};

/// Live transparency view for coding threads. Subscribes to the
/// `agent:thread_event` stream and renders every item — user message,
/// reasoning, tool call, tool result, file change, command output —
/// inline as it arrives.
///
/// `draftPrompt` lets the composer hand off the user's text so the message
/// appears immediately, without waiting for the backend echo (avoids the
/// listener-registration race where the first ItemStarted gets dropped).
export function CodingThreadView({ threadId, draftPrompt, onDraftConsumed }: Props) {
  const { items, turnState, pushUserMessage } = useThreadEvents(threadId);
  const lastDraftRef = useRef<string | null>(null);

  useEffect(() => {
    if (!draftPrompt || !threadId) return;
    if (lastDraftRef.current === draftPrompt) return;
    lastDraftRef.current = draftPrompt;
    pushUserMessage(draftPrompt);
    onDraftConsumed?.();
  }, [draftPrompt, threadId, pushUserMessage, onDraftConsumed]);

  if (!threadId) return null;

  return (
    <div className="coding-thread-view">
      <ThreadItemList items={items} threadId={threadId} />
      {turnState.kind !== "idle" && (
        <div className="coding-thread-view__status" role="status">
          {turnState.kind === "streaming" && <>Streaming from {turnState.model}…</>}
          {turnState.kind === "tool_executing" && <>Running tool {turnState.tool}…</>}
        </div>
      )}
    </div>
  );
}
