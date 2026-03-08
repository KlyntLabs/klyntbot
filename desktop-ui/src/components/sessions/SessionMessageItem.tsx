import { useMemo } from "react";
import type { ContentBlock, SessionMessage } from "../../lib/session-types";
import { MarkdownContent } from "../chat/MarkdownContent";

interface SessionMessageItemProps {
  message: SessionMessage;
  onPin?: (messageUuid: string) => void;
}

export function SessionMessageItem({ message, onPin }: SessionMessageItemProps) {
  // Hooks must be called unconditionally (before any early returns)
  const text = useMemo(() => {
    if (Array.isArray(message.content)) return extractText(message.content);
    if (typeof message.content === "string") return message.content;
    return message.text ?? "";
  }, [message.content, message.text]);

  if (message.type === "progress") {
    return (
      <div className="flex justify-center py-0.5">
        <span className="glass-badge text-[10px] text-dim font-light px-2 py-0.5">
          {message.subtype ?? "working..."}
        </span>
      </div>
    );
  }

  if (message.type === "system") {
    return (
      <div className="flex justify-center py-1">
        <span className="text-[11px] text-dim font-light italic">{message.text ?? "system"}</span>
      </div>
    );
  }

  if (message.type === "queueOperation") {
    const queueText = typeof message.content === "string" ? message.content : message.text;
    if (message.operation === "enqueue" && queueText) {
      return (
        <div className="group flex justify-end py-1">
          <div className="max-w-[85%] px-4 py-2.5 text-[13px] font-light glass-bubble-user text-primary">
            <p className="whitespace-pre-wrap break-words">{queueText}</p>
          </div>
        </div>
      );
    }
    return null;
  }

  const isUser = message.type === "user";

  // Skip assistant messages with no text content (pure tool calls)
  if (!isUser && !text) return null;

  if (isUser) {
    return (
      <div className="group flex justify-end py-1">
        <div className="max-w-[85%] glass-bubble-user px-5 py-3.5">
          <p className="text-[13px] font-light whitespace-pre-wrap leading-relaxed text-primary">
            {message.text ?? (typeof message.content === "string" ? message.content : "")}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="group flex justify-start items-start gap-2 py-0.5">
      <PinMarker canPin={!!onPin && !!message.uuid} onPin={() => onPin?.(message.uuid!)} />
      <div className="max-w-[85%]">{text && <MarkdownContent content={text} />}</div>
    </div>
  );
}

function PinMarker({ canPin, onPin }: { canPin: boolean; onPin: () => void }) {
  if (!canPin) {
    return <span className="mt-[5px] w-2 h-2 shrink-0 rounded-full border border-brand/40" />;
  }

  return (
    <button
      type="button"
      onClick={onPin}
      title="Pin message"
      className="mt-[5px] w-2 h-2 shrink-0 rounded-full border border-brand/40 transition-colors hover:border-brand hover:bg-brand/40"
    />
  );
}

/** Extract only text content from ContentBlock[], skipping tool_use/tool_result. */
function extractText(blocks: ContentBlock[]): string {
  return blocks
    .filter((b) => b.type === "text" && b.text)
    .map((b) => b.text!)
    .join("\n\n");
}
