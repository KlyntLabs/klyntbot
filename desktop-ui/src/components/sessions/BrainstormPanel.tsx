import { Send } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useAutoResizeTextarea } from "../../hooks/useAutoResizeTextarea";
import { useBrainstormStream } from "../../hooks/useBrainstormStream";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { BrainstormMessage, PinnedMessage, TrackedSession } from "../../lib/session-types";
import { ActionBar } from "./ActionBar";
import { PinnedContextStrip } from "./PinnedContextStrip";
import { SendConfirmDialog } from "./SendConfirmDialog";

interface BrainstormPanelProps {
  conversationId: string;
  sessionId: string;
  session: TrackedSession;
}

export function BrainstormPanel({ conversationId, sessionId, session }: BrainstormPanelProps) {
  const { messages, segments, isStreaming, send } = useBrainstormStream(conversationId);
  const { data: pins, refetch: refetchPins } = useQuery<PinnedMessage[]>(
    "get_pinned_messages",
    { sessionId },
    [],
  );

  const unpinMutation = useMutation<void, { pinId: number }>("unpin_session_message");

  const editMutation = useMutation<void, { messageId: string; editedContent: string }>(
    "edit_brainstorm_message",
  );

  const sendToCcMutation = useMutation<void, { sessionId: string; content: string }>(
    "send_to_claude_code",
  );

  const [input, setInput] = useState("");
  const [sendDialogContent, setSendDialogContent] = useState<string | null>(null);

  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);
  const scrollRef = useRef<HTMLDivElement>(null);

  const handleSend = useCallback(() => {
    const text = input.trim();
    if (!text || isStreaming) return;
    setInput("");
    send(text);
  }, [input, isStreaming, send]);

  const handleUnpin = useCallback(
    async (pinId: number) => {
      await unpinMutation.mutate({ pinId });
      await refetchPins();
    },
    [unpinMutation.mutate, refetchPins],
  );

  const handleEdit = useCallback(
    async (messageId: string, editedContent: string) => {
      await editMutation.mutate({ messageId, editedContent });
    },
    [editMutation.mutate],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger scroll on new content
  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages.length, segments.length]);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Pinned context */}
      <PinnedContextStrip pins={pins} onUnpin={handleUnpin} />

      {/* Messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-2">
        {messages.map((msg) => (
          <BrainstormMessageItem
            key={msg.id}
            message={msg}
            onEdit={(content) => handleEdit(msg.id, content)}
            onSend={(content) => setSendDialogContent(content)}
          />
        ))}
        {/* Streaming segment */}
        {segments.length > 0 && (
          <div className="glass-bubble px-4 py-2.5 text-[13px] font-light text-primary">
            <p className="whitespace-pre-wrap break-words">{segments[0].content}</p>
          </div>
        )}
        {isStreaming && segments.length === 0 && (
          <div className="flex items-center gap-1.5 px-2 py-2">
            <span
              className="w-1.5 h-1.5 rounded-full bg-brand animate-bounce"
              style={{ animationDelay: "0ms" }}
            />
            <span
              className="w-1.5 h-1.5 rounded-full bg-brand animate-bounce"
              style={{ animationDelay: "150ms" }}
            />
            <span
              className="w-1.5 h-1.5 rounded-full bg-brand animate-bounce"
              style={{ animationDelay: "300ms" }}
            />
          </div>
        )}
      </div>

      {/* Input */}
      <div className="p-4">
        <div className="flex gap-2 items-end">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onInput={handleInput}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSend();
              }
            }}
            aria-label="Brainstorm message"
            placeholder="Brainstorm about this session..."
            rows={1}
            className="flex-1 glass-input px-4 py-2.5 text-[13px] text-primary placeholder:text-muted font-light resize-none overflow-hidden outline-none"
            style={{ maxHeight: "120px" }}
          />
          <button
            type="button"
            onClick={handleSend}
            disabled={!input.trim() || isStreaming}
            aria-label="Send message"
            className="w-10 h-10 rounded-xl bg-brand hover:bg-brand-hover disabled:bg-white/[0.06] disabled:text-muted flex items-center justify-center transition-all shrink-0"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
      </div>

      {/* Send confirm dialog */}
      {sendDialogContent != null && (
        <SendConfirmDialog
          open
          session={session}
          content={sendDialogContent}
          onClose={() => setSendDialogContent(null)}
          onConfirm={async () => {
            await sendToCcMutation.mutate({
              sessionId,
              content: sendDialogContent,
            });
            setSendDialogContent(null);
          }}
        />
      )}
    </div>
  );
}

function BrainstormMessageItem({
  message,
  onEdit,
  onSend,
}: {
  message: BrainstormMessage;
  onEdit: (content: string) => void;
  onSend: (content: string) => void;
}) {
  const isUser = message.role === "user";
  const displayContent = message.editedContent ?? message.content;

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[85%] px-4 py-2.5 text-[13px] font-light ${
          isUser ? "glass-bubble-user text-primary" : "glass-bubble text-primary"
        }`}
      >
        <p className="whitespace-pre-wrap break-words">{displayContent}</p>
        {message.isResultBlock && !isUser && (
          <div className="mt-2 pt-2 border-t border-white/[0.08]">
            <ActionBar content={displayContent} onEdit={onEdit} onSend={onSend} />
          </div>
        )}
      </div>
    </div>
  );
}
