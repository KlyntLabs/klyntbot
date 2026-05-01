import type { ComponentProps } from "react";
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { Composer } from "@/features/composer/components/Composer";
import type { Messages } from "@/features/messages/components/Messages";
import type {
  RequestUserInputRequest,
  RequestUserInputResponse,
  ConversationItem,
} from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { chatStreamStore } from "../store/chatStreamStore";
import { useChatSession } from "./useChatSession";
import type {
  ChatMessage,
  MessageSegment,
  ActiveInteraction,
  FormResponse,
  Answer,
  Question,
} from "../types";

type MessagesProps = ComponentProps<typeof Messages>;
type ComposerProps = ComponentProps<typeof Composer>;

type MessagesOverride = Pick<
  MessagesProps,
  | "items"
  | "threadId"
  | "isThinking"
  | "processingStartedAt"
  | "userInputRequests"
  | "onUserInputSubmit"
>;

type ComposerOverride = Pick<
  ComposerProps,
  | "onSend"
  | "onStop"
  | "canStop"
  | "isProcessing"
  | "models"
  | "selectedModelId"
  | "onSelectModel"
  | "collaborationModes"
  | "selectedCollaborationModeId"
  | "onSelectCollaborationMode"
  | "reasoningSupported"
  | "accessMode"
  | "onSelectAccessMode"
  | "attachedImages"
  | "onPickImages"
  | "onAttachImages"
  | "onRemoveImage"
  | "dictationEnabled"
  | "queuedMessages"
  | "draftText"
  | "onDraftChange"
  | "historyKey"
>;

export type KlyntbotSurfaceOverrides = {
  messagesProps: MessagesOverride;
  composerProps: ComposerOverride;
  error: string | null;
  onDismissError: () => void;
};

function buildItems(
  messages: ChatMessage[],
  segments: MessageSegment[],
  approvals: ConversationItem[],
): ConversationItem[] {
  const items: ConversationItem[] = messages.map((msg) => ({
    id: msg.id,
    kind: "message" as const,
    role: msg.role === "user" ? ("user" as const) : ("assistant" as const),
    text: msg.content,
  }));

  let textBuffer = "";
  let textBufferIndex = -1;

  segments.forEach((seg, idx) => {
    if (seg.type === "text") {
      if (textBufferIndex === -1) textBufferIndex = idx;
      textBuffer += seg.content;
      return;
    }
    if (seg.type !== "tool") {
      return;
    }
    if (textBuffer.length > 0) {
      items.push({
        id: `stream-assistant-${textBufferIndex}`,
        kind: "message",
        role: "assistant",
        text: textBuffer,
      });
      textBuffer = "";
      textBufferIndex = -1;
    }
    items.push({
      id: `stream-tool-${idx}`,
      kind: "tool",
      toolType: seg.name,
      title: seg.name,
      detail: seg.action ?? "",
      output: seg.result,
      durationMs: seg.durationMs,
      status: seg.success ? "completed" : "failed",
    });
  });

  if (textBuffer.length > 0) {
    items.push({
      id: `stream-assistant-${textBufferIndex}`,
      kind: "message",
      role: "assistant",
      text: textBuffer,
    });
  }

  if (approvals.length > 0) {
    items.push(...approvals);
  }

  return items;
}

function buildUserInputRequests(
  active: ActiveInteraction | null,
  sessionKey: string,
): RequestUserInputRequest[] {
  if (!active) return [];
  return [
    {
      workspace_id: "",
      request_id: active.requestId,
      params: {
        thread_id: sessionKey,
        turn_id: "",
        item_id: active.requestId,
        questions: active.request.questions.map((q) => ({
          id: q.id,
          header: q.title,
          question: q.text,
        })),
      },
    },
  ];
}

function toFormResponse(
  response: RequestUserInputResponse,
  questions: Question[],
): FormResponse {
  const answers: Answer[] = Object.entries(response.answers).map(
    ([questionId, ans]) => {
      const question = questions.find((q) => q.id === questionId);
      const answerType = question?.answer_type?.type ?? "free_text";
      switch (answerType) {
        case "single_select":
          return {
            question_id: questionId,
            value: { type: "selected", value: ans.answers[0] ?? "" },
          };
        case "multi_select":
          return {
            question_id: questionId,
            value: { type: "multi_selected", values: ans.answers },
          };
        case "yes_no":
          return {
            question_id: questionId,
            value: {
              type: "yes_no",
              answer: ans.answers[0] === "true" || ans.answers[0] === "yes",
            },
          };
        case "free_text":
        default:
          return {
            question_id: questionId,
            value: { type: "text", content: ans.answers.join(", ") },
          };
      }
    },
  );
  return { Completed: answers };
}

export function useKlyntbotSurfaceProps(
  sessionKey: string | null,
): KlyntbotSurfaceOverrides | null {
  const chat = useChatSession(sessionKey ?? "");

  const processingStartedAtRef = useRef<number | null>(
    chat.isStreaming ? Date.now() : null,
  );
  const prevIsStreamingRef = useRef(chat.isStreaming);
  if (!prevIsStreamingRef.current && chat.isStreaming) {
    processingStartedAtRef.current = Date.now();
  } else if (prevIsStreamingRef.current && !chat.isStreaming) {
    processingStartedAtRef.current = null;
  }
  prevIsStreamingRef.current = chat.isStreaming;

  const [dismissedError, setDismissedError] = useState<string | null>(null);
  useEffect(() => {
    if (dismissedError !== null && chat.error !== dismissedError) {
      setDismissedError(null);
    }
  }, [chat.error, dismissedError]);
  const visibleError = dismissedError === chat.error ? null : chat.error;

  // Hooks MUST always run in the same order — never early-return between them.
  // sessionKey may be null (no thread selected); we still register the
  // subscription so React's hook count is stable, then bail out below.
  const approvals = useSyncExternalStore(
    chatStreamStore.subscribe,
    useCallback(() => chatStreamStore.getApprovals(sessionKey ?? ""), [sessionKey]),
  );

  if (!sessionKey) {
    return null;
  }

  return {
    messagesProps: {
      items: buildItems(chat.messages, chat.segments, approvals),
      threadId: sessionKey,
      isThinking: chat.isStreaming,
      processingStartedAt: processingStartedAtRef.current,
      userInputRequests: buildUserInputRequests(chat.activeInteraction, sessionKey),
      onUserInputSubmit: (request, response) => {
        if (!chat.activeInteraction) return;
        void invoke("chat_respond_interaction", {
          sessionKey,
          requestId: String(request.request_id),
          response: toFormResponse(response, chat.activeInteraction.request.questions),
        });
      },
    },
    composerProps: {
      onSend: () => {
        void chat.send();
      },
      onStop: () => {},
      canStop: false,
      isProcessing: chat.isStreaming,
      models: [{ id: "klyntbot", displayName: "klyntbot", model: "klyntbot" }],
      selectedModelId: "klyntbot",
      onSelectModel: () => {},
      collaborationModes: [{ id: "default", label: "klyntbot" }],
      selectedCollaborationModeId: "default",
      onSelectCollaborationMode: () => {},
      reasoningSupported: false,
      accessMode: "current",
      onSelectAccessMode: () => {},
      attachedImages: [],
      onPickImages: () => {},
      onAttachImages: () => {},
      onRemoveImage: () => {},
      dictationEnabled: false,
      queuedMessages: [],
      draftText: chat.input,
      onDraftChange: chat.setInput,
      historyKey: sessionKey,
    },
    error: visibleError,
    onDismissError: () => setDismissedError(chat.error),
  };
}
