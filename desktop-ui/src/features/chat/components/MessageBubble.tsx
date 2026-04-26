import { Markdown } from "@/features/messages/components/Markdown";
import type { ChatMessage } from "../types";

type MessageBubbleProps = {
  message: ChatMessage;
};

export function MessageBubble({ message }: MessageBubbleProps) {
  const role = message.role;
  return (
    <div className={`chat-bubble chat-bubble--${role}`} data-role={role}>
      {role === "user" ? (
        <div className="chat-bubble__user-text">{message.content}</div>
      ) : (
        <Markdown value={message.content} className="chat-bubble__markdown" />
      )}
    </div>
  );
}
