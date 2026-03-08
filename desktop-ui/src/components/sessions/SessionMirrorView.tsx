import { useCallback, useEffect, useRef } from "react";
import { useSessionStream } from "../../hooks/useSessionStream";
import { SessionMessageItem } from "./SessionMessageItem";

interface SessionMirrorViewProps {
  sessionId: string;
  showPinButtons?: boolean;
}

export function SessionMirrorView({ sessionId, showPinButtons }: SessionMirrorViewProps) {
  const { messages, isLive, pinMessage } = useSessionStream(sessionId);

  const handlePin = useCallback(
    (uuid: string) => {
      const msg = messages.find((m) => m.uuid === uuid);
      if (!msg) return;
      const content = msg.text ?? (typeof msg.content === "string" ? msg.content : "");
      pinMessage(uuid, content, msg.type);
    },
    [messages, pinMessage],
  );
  const scrollRef = useRef<HTMLDivElement>(null);
  const userScrolledUpRef = useRef(false);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    userScrolledUpRef.current = el.scrollHeight - el.scrollTop - el.clientHeight > 100;
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: trigger scroll on new messages
  useEffect(() => {
    const el = scrollRef.current;
    if (el && !userScrolledUpRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages.length]);

  return (
    <div ref={scrollRef} onScroll={handleScroll} className="flex-1 overflow-y-auto p-5">
      {messages.length === 0 && isLive ? (
        <div className="flex items-center justify-center py-12">
          <span className="text-muted text-[12px] font-light">No messages yet</span>
        </div>
      ) : (
        <div className="space-y-1.5">
          {messages.map((msg, i) => (
            <SessionMessageItem
              key={msg.uuid ?? `${msg.type}-${i}`}
              message={msg}
              onPin={showPinButtons ? handlePin : undefined}
            />
          ))}
        </div>
      )}
    </div>
  );
}
