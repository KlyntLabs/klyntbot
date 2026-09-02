import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";
import { useChatSession } from "@shared/hooks/useChatSession";
import { useMutation } from "@shared/hooks/useMutation";
import { usePageContext } from "@shared/hooks/usePageContext";
import { Pin, Send, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { MessageList } from "./MessageList";

interface SidebarChatProps {
  isOpen: boolean;
  onClose: () => void;
  /** View-level context from the parent (e.g. active sidebar section + tab). */
  viewContext?: { entityKind: string; entityId?: string };
  /** Session key passed from the launcher expand-to-main flow. */
  openSessionKey?: string | null;
  /** Callback to signal that the session key has been consumed. */
  onSessionKeyUsed?: () => void;
}

/** Derive a stable session key from page context. */
function sessionKeyFor(entityKind?: string, entityId?: string): string {
  if (!entityKind) return "desktop-panel";
  return `sidebar:${entityKind}:${entityId || "all"}`;
}

/** Human-readable label for the sidebar header. */
function contextLabel(entityKind?: string): string {
  if (!entityKind) return "Chat";
  const parts = entityKind.split(".");
  return parts.map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join(" \u203A ");
}

export function SidebarChat({
  isOpen,
  onClose,
  viewContext,
  openSessionKey,
  onSessionKeyUsed,
}: SidebarChatProps) {
  const routeContext = usePageContext();
  // Route context (entity detail pages) takes priority; fall back to view context
  const pageContext = routeContext ?? viewContext ?? null;

  // Override session key when the launcher expands a chat to main
  const [overrideSessionKey, setOverrideSessionKey] = useState<string | null>(null);

  useEffect(() => {
    if (openSessionKey) {
      setOverrideSessionKey(openSessionKey);
      onSessionKeyUsed?.();
    }
  }, [openSessionKey, onSessionKeyUsed]);

  // Clear override when route context changes to a new value (user navigates)
  const prevRouteRef = useRef(routeContext);
  useEffect(() => {
    const prev = prevRouteRef.current;
    if (
      routeContext &&
      (routeContext.entityKind !== prev?.entityKind || routeContext.entityId !== prev?.entityId)
    ) {
      setOverrideSessionKey(null);
    }
    prevRouteRef.current = routeContext;
  }, [routeContext]);

  // Use override session key if set, otherwise derive from route context
  const sessionKey =
    overrideSessionKey ??
    (routeContext
      ? sessionKeyFor(routeContext.entityKind, routeContext.entityId)
      : "desktop-panel");

  const chat = useChatSession(sessionKey, undefined, undefined);
  const pinThread = useMutation<void, Record<string, unknown>>("chat_pin_thread");

  const handleSend = () => {
    chat.send(pageContext ? { context: { ...pageContext, isEphemeral: true } } : undefined);
  };

  const handlePin = async () => {
    await pinThread.mutate({ sessionKey });
  };

  if (!isOpen) return null;

  const label = contextLabel(pageContext?.entityKind);

  return (
    <div className="w-96 glass-sidebar flex flex-col shrink-0">
      {/* Header */}
      <div className="h-14 flex items-center justify-between px-5">
        <span className="text-ui font-light text-fg-secondary">{label}</span>
        <div className="flex items-center gap-1">
          {pageContext && (
            <button
              type="button"
              onClick={handlePin}
              aria-label="Pin to Chat threads"
              title="Pin to Chat threads"
              className="size-7 rounded-lg flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-all"
            >
              <Pin className="size-3.5" strokeWidth={1.5} />
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            aria-label="Close chat"
            className="size-7 rounded-lg flex items-center justify-center text-fg-secondary hover:text-fg hover:bg-control-hover transition-all"
          >
            <X className="size-4" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      <div className="mx-4 glass-divider" />

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-5">
        {chat.messages.length === 0 && !chat.isStreaming ? (
          <div className="flex flex-col items-center justify-center py-12">
            <p className="text-fg-secondary text-ui-sm font-light text-center">
              {pageContext
                ? `Ask about this ${pageContext.entityKind?.split(".")[0] || "item"}`
                : "Ask me anything"}
            </p>
          </div>
        ) : (
          <>
            <MessageList
              messages={chat.messages}
              segments={chat.segments}
              isStreaming={chat.isStreaming}
              activeTools={chat.activeTools}
              error={chat.error}
              activeInteraction={chat.activeInteraction}
              sessionKey={sessionKey}
              onInteractionSubmitted={() => chat.clearInteraction()}
              liveTransparency={null}
              activeDelegateAgent={chat.activeDelegateAgent}
              statusPhase={chat.statusPhase}
            />
          </>
        )}
      </div>

      {/* Input */}
      <SidebarChatInput
        input={chat.input}
        isStreaming={chat.isStreaming}
        onInputChange={chat.setInput}
        onSend={handleSend}
      />
    </div>
  );
}

function SidebarChatInput({
  input,
  isStreaming,
  onInputChange,
  onSend,
}: {
  input: string;
  isStreaming: boolean;
  onInputChange: (value: string) => void;
  onSend: () => void;
}) {
  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);

  return (
    <div className="p-4">
      <div className="flex gap-2 items-end">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => onInputChange(e.target.value)}
          onInput={handleInput}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              onSend();
            }
          }}
          aria-label="Message input"
          placeholder="Ask about this page\u2026"
          rows={1}
          className="flex-1 glass-input px-4 py-2.5 text-ui text-fg placeholder:text-fg-secondary font-light resize-none overflow-hidden outline-none"
          style={{ maxHeight: "120px" }}
        />
        <button
          type="button"
          onClick={onSend}
          disabled={!input.trim() || isStreaming}
          aria-label="Send message"
          className="size-10 rounded-xl bg-brand text-brand-foreground hover:bg-brand-hover disabled:bg-control-hover disabled:text-fg-secondary flex items-center justify-center transition-all shrink-0"
        >
          <Send className="size-4" strokeWidth={2} />
        </button>
      </div>
    </div>
  );
}
