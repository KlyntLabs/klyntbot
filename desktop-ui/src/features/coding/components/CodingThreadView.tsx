import { useEffect, useMemo, useRef } from "react";
import { Messages } from "@/features/messages/components/Messages";
import type { ConversationItem, OpenAppTarget } from "@/types";
import { type MessageDto, useThreadEvents } from "../hooks/useThreadEvents";
import type { MessagePart } from "./parts/types";
import { PlanModeBanner } from "./PlanModeBanner";
import { TodoPanel } from "./TodoPanel";
import { JobsPanel } from "./JobsPanel";

type Props = {
  threadId: string | null;
  workspaceId?: string | null;
  workspacePath?: string | null;
  openTargets: OpenAppTarget[];
  selectedOpenAppId: string;
  draftPrompt?: string | null;
  onDraftConsumed?: () => void;
};

/// Live transparency view for coding threads.
///
/// Subscribes to the new `agent:thread_event` stream via `useThreadEvents`,
/// adapts each MessageDto into the legacy `ConversationItem` shape, then
/// renders through the polished `<Messages>` component (avatars, bubbles,
/// markdown, code blocks, copy buttons — all the UX assistant-mode users
/// already get). Single rendering path, two event sources.
export function CodingThreadView({
  threadId,
  workspaceId = null,
  workspacePath = null,
  openTargets,
  selectedOpenAppId,
  draftPrompt,
  onDraftConsumed,
}: Props) {
  const { items, turnState, pushUserMessage } = useThreadEvents(threadId);
  const lastDraftRef = useRef<string | null>(null);

  useEffect(() => {
    if (!draftPrompt || !threadId) return;
    if (lastDraftRef.current === draftPrompt) return;
    lastDraftRef.current = draftPrompt;
    pushUserMessage(draftPrompt);
    onDraftConsumed?.();
  }, [draftPrompt, threadId, pushUserMessage, onDraftConsumed]);

  const conversationItems = useMemo(() => adaptItems(items), [items]);
  const isThinking = turnState.kind !== "idle";

  if (!threadId) return null;

  return (
    <div className="flex flex-col h-full">
      <PlanModeBanner threadId={threadId} />
      <div className="flex flex-1 overflow-hidden">
        <div className="flex-1 min-w-0">
          <Messages
            items={conversationItems}
            threadId={threadId}
            workspaceId={workspaceId}
            workspacePath={workspacePath}
            openTargets={openTargets}
            selectedOpenAppId={selectedOpenAppId}
            isThinking={isThinking}
          />
        </div>
        <div className="w-64 border-l border-border hidden lg:block flex flex-col">
          <TodoPanel threadId={threadId} />
          <JobsPanel threadId={threadId} />
        </div>
      </div>
    </div>
  );
}

/// Translate the new MessageDto stream into the assistant-mode
/// `ConversationItem` shape. We collapse `text`/`reasoning` parts into a
/// single message bubble, surface tool calls + results as their own items,
/// and emit diff/command items for file/command parts.
function adaptItems(items: MessageDto[]): ConversationItem[] {
  const out: ConversationItem[] = [];
  for (const item of items) {
    // Buffers accumulate consecutive same-kind parts, then flush in-order
    // when a different-kind part arrives — this keeps the natural temporal
    // ordering the model produced (reasoning → tool_call → reasoning → …)
    // instead of clumping all reasoning at the start or end of the message.
    let textBuffer = "";
    let reasoningBuffer = "";
    let bufferIdx = 0;
    const flushBuffers = () => {
      if (reasoningBuffer) {
        // Real reasoning_content from extended-thinking models (Kimi-Reasoner,
        // DeepSeek-R1, qwq, glm-zero, etc.) — renders as italic "Thinking:"
        // inline prose. Distinct from regular text content the model emits as
        // its visible answer.
        out.push({
          id: `${item.id}-reasoning-${bufferIdx}`,
          kind: "reasoning",
          summary: "Thinking",
          content: reasoningBuffer,
        });
        reasoningBuffer = "";
      }
      if (textBuffer) {
        out.push({
          id: `${item.id}-text-${bufferIdx}`,
          kind: "message",
          role: item.role === "user" ? "user" : "assistant",
          text: textBuffer,
        });
        textBuffer = "";
      }
      bufferIdx += 1;
    };
    for (const raw of item.parts) {
      const part = raw as MessagePart;
      switch (part.kind) {
        case "text":
          textBuffer += part.text;
          break;
        case "reasoning":
          reasoningBuffer += part.text;
          break;
        case "tool_call":
          flushBuffers();
          out.push({
            id: `${item.id}-call-${part.call_id}`,
            kind: "tool",
            toolType: part.name,
            title: part.name,
            detail: previewArgs(part.args),
          });
          break;
        case "tool_result":
          flushBuffers();
          out.push({
            id: `${item.id}-result-${part.call_id}`,
            kind: "tool",
            toolType: "result",
            title: "result",
            detail: part.output.text.slice(0, 200),
            output: part.output.text,
            status: part.is_error ? "error" : "ok",
          });
          break;
        case "file_change":
          flushBuffers();
          out.push({
            id: `${item.id}-file-${part.path}`,
            kind: "diff",
            title: part.path,
            diff: part.diff_unified || "",
            path: part.path,
          });
          break;
        case "command_execution":
          flushBuffers();
          out.push({
            id: `${item.id}-cmd-${item.parts.indexOf(raw)}`,
            kind: "tool",
            toolType: "bash",
            title: part.command.join(" "),
            detail: part.stdout.slice(0, 200),
            output: [part.stdout, part.stderr].filter(Boolean).join("\n"),
            status: part.exit_code === 0 ? "ok" : "error",
          });
          break;
        default:
          break;
      }
    }
    flushBuffers();
  }
  return out;
}

function previewArgs(args: unknown): string {
  try {
    const s = typeof args === "string" ? args : JSON.stringify(args);
    return s.length > 200 ? `${s.slice(0, 200)}…` : s;
  } catch {
    return "";
  }
}
