import { Pin } from "lucide-react";
import type { ContentBlock, SessionMessage } from "../../lib/session-types";
import { CollapsibleToolBlock } from "./CollapsibleToolBlock";

interface SessionMessageItemProps {
  message: SessionMessage;
  onPin?: (messageUuid: string) => void;
}

function renderContentBlocks(content: ContentBlock[]) {
  const elements: React.ReactNode[] = [];
  for (let i = 0; i < content.length; i++) {
    const block = content[i];
    if (block.type === "text" && block.text) {
      elements.push(
        <p key={`text-${i}`} className="whitespace-pre-wrap break-words">
          {block.text}
        </p>,
      );
    } else if (block.type === "tool_use") {
      const next = content[i + 1];
      const toolResult =
        next?.type === "tool_result" && next.toolUseId === block.id ? next : undefined;
      if (toolResult) i++; // skip the consumed tool_result
      elements.push(
        <CollapsibleToolBlock key={block.id ?? `tool-${i}`} tool={block} result={toolResult} />,
      );
    }
    // standalone tool_result blocks without a preceding tool_use are silently skipped
  }
  return elements;
}

export function SessionMessageItem({ message, onPin }: SessionMessageItemProps) {
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
    // Enqueue with content = user sent a message while Claude was working
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
    // Hide dequeue/remove operations — they're just bookkeeping
    return null;
  }

  const isUser = message.type === "user";

  return (
    <div className={`group flex ${isUser ? "justify-end" : "justify-start"} py-1`}>
      <div
        className={`max-w-[85%] px-4 py-2.5 text-[13px] font-light ${
          isUser ? "glass-bubble-user text-primary" : "glass-bubble text-primary"
        }`}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap break-words">{message.text}</p>
        ) : (
          <AssistantContent message={message} />
        )}
      </div>
      {onPin && message.uuid && (
        <button
          type="button"
          onClick={() => onPin(message.uuid as string)}
          title="Pin message"
          className="self-start mt-2 ml-1 opacity-0 group-hover:opacity-100 transition-opacity w-6 h-6 rounded-md flex items-center justify-center text-dim hover:text-muted"
        >
          <Pin className="w-3 h-3" strokeWidth={1.5} />
        </button>
      )}
    </div>
  );
}

function AssistantContent({ message }: { message: SessionMessage }) {
  if (Array.isArray(message.content)) {
    return <div className="space-y-1">{renderContentBlocks(message.content)}</div>;
  }
  const display = typeof message.content === "string" ? message.content : message.text;
  return display ? (
    <div className="space-y-1">
      <p className="whitespace-pre-wrap break-words">{display}</p>
    </div>
  ) : null;
}
