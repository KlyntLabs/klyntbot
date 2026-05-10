import { useEffect, useMemo, useRef } from "react";
import { Messages } from "@/features/messages/components/Messages";
import type { ConversationItem, OpenAppTarget } from "@/types";
import { type MessageDto, useThreadEvents } from "../hooks/useThreadEvents";
import { PlanModeBanner } from "./PlanModeBanner";
import type { MessagePart } from "./parts/types";

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
  // Pin the seed per-threadId so onDraftConsumed clearing the parent's
  // map doesn't retroactively wipe the optimistic message in useThreadEvents.
  const seededThreadRef = useRef<string | null>(null);
  const seededPromptRef = useRef<string | null>(null);
  if (threadId && seededThreadRef.current !== threadId) {
    seededThreadRef.current = threadId;
    seededPromptRef.current = draftPrompt ?? null;
  }
  const { items, turnState } = useThreadEvents(threadId, seededPromptRef.current);

  useEffect(() => {
    if (!draftPrompt || !threadId) return;
    if (seededPromptRef.current !== draftPrompt) return;
    onDraftConsumed?.();
  }, [draftPrompt, threadId, onDraftConsumed]);

  const conversationItems = useMemo(() => adaptItems(items), [items]);
  const isThinking = turnState.kind !== "idle";

  if (!threadId) return null;

  return (
    <div className="coding-thread-view">
      <PlanModeBanner threadId={threadId} />
      <div className="coding-thread-view__body">
        <div className="coding-thread-view__main">
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
      </div>
    </div>
  );
}

/// Translate the new MessageDto stream into the assistant-mode
/// `ConversationItem` shape. We collapse `text`/`reasoning` parts into a
/// single message bubble, surface tool calls + results as their own items,
/// and emit diff/command items for file/command parts.
function flattenAssistantText(item: MessageDto): string | null {
  if (item.role !== "assistant") return null;
  let only: string | null = null;
  for (const raw of item.parts) {
    const p = raw as MessagePart;
    if (p.kind === "text") {
      only = (only ?? "") + p.text;
    } else if (p.kind !== "reasoning") {
      // Has non-text payload (tool/diff/etc) — not a pure-text row.
      return null;
    }
  }
  return only;
}

/// Drop consecutive same-role assistant rows whose Text content is a
/// substring/superset of the next — symptom of streaming snapshot + final
/// flush both being persisted as separate rows in turn_handler.
function dedupePrefixDuplicates(items: MessageDto[]): MessageDto[] {
  const out: MessageDto[] = [];
  let skipNext = false;
  for (let i = 0; i < items.length; i += 1) {
    if (skipNext) {
      skipNext = false;
      continue;
    }
    const cur = items[i];
    const next = items[i + 1];
    const curText = flattenAssistantText(cur);
    const nextText = next && next.role === cur.role ? flattenAssistantText(next) : null;
    if (curText !== null && nextText !== null) {
      const a = curText.trim();
      const b = nextText.trim();
      if (a === b || a.includes(b) || b.includes(a)) {
        // Keep the longer (it's a superset of the shorter).
        out.push(curText.length >= nextText.length ? cur : next);
        skipNext = true;
        continue;
      }
    }
    out.push(cur);
  }
  return out;
}

/// The AGENTS.md bundle is persisted as a synthetic user message at thread
/// start (see `coding-agents-md::WorkspaceAgentsSource`). The LLM still needs
/// it as context, so the row stays in the DB — but it's noisy as a chat
/// bubble. Surface it through the prompts sidebar instead.
function isAgentsMdSyntheticMessage(item: MessageDto): boolean {
  if (item.role !== "user") return false;
  if (item.parts.length !== 1) return false;
  const only = item.parts[0] as MessagePart;
  return only.kind === "text" && only.text.startsWith("# AGENTS.md instructions for ");
}

function adaptItems(rawItems: MessageDto[]): ConversationItem[] {
  const items = dedupePrefixDuplicates(rawItems).filter(
    (item) => !isAgentsMdSyntheticMessage(item),
  );
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
