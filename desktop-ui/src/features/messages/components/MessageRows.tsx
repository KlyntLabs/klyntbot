import type { ParsedFileLocation } from "@utils/fileLinks";
import Check from "lucide-react/dist/esm/icons/check";
import Copy from "lucide-react/dist/esm/icons/copy";
import Quote from "lucide-react/dist/esm/icons/quote";
import X from "lucide-react/dist/esm/icons/x";
import type { MouseEvent } from "react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { PierreDiffBlock } from "@/features/git/components/PierreDiffBlock";
import type { ConversationItem } from "@/types";
import {
  exploreKindLabel,
  formatDurationMs,
  type MessageImage,
  normalizeMessageImageSrc,
  type ParsedReasoning,
  toolRowDescriptor,
  toolStatusTone,
} from "../utils/messageRenderUtils";
import { BashTail } from "./BashTail";
import { isStandaloneMarkdownTable, Markdown } from "./Markdown";
import { ToolRowBody } from "./ToolRowBody";

type MarkdownFileLinkProps = {
  showMessageFilePath?: boolean;
  workspacePath?: string | null;
  onOpenFileLink?: (path: ParsedFileLocation) => void;
  onOpenFileLinkMenu?: (event: MouseEvent, path: ParsedFileLocation) => void;
  onOpenThreadLink?: (threadId: string) => void;
};

type WorkingIndicatorProps = {
  isThinking: boolean;
  processingStartedAt?: number | null;
  lastDurationMs?: number | null;
  hasItems: boolean;
  reasoningLabel?: string | null;
  showPollingFetchStatus?: boolean;
  pollingIntervalMs?: number;
};

type MessageRowProps = MarkdownFileLinkProps & {
  item: Extract<ConversationItem, { kind: "message" }>;
  isCopied: boolean;
  onCopy: (item: Extract<ConversationItem, { kind: "message" }>) => void;
  onQuote?: (item: Extract<ConversationItem, { kind: "message" }>, selectedText?: string) => void;
  codeBlockCopyUseModifier?: boolean;
};

type ReasoningRowProps = MarkdownFileLinkProps & {
  item: Extract<ConversationItem, { kind: "reasoning" }>;
  parsed: ParsedReasoning;
};

type ReviewRowProps = MarkdownFileLinkProps & {
  item: Extract<ConversationItem, { kind: "review" }>;
};

type DiffRowProps = {
  item: Extract<ConversationItem, { kind: "diff" }>;
};

type UserInputRowProps = {
  item: Extract<ConversationItem, { kind: "userInput" }>;
  isExpanded: boolean;
  onToggle: (id: string) => void;
};

type ToolRowProps = MarkdownFileLinkProps & {
  item: Extract<ConversationItem, { kind: "tool" }>;
  isExpanded: boolean;
  onToggle: (id: string) => void;
  onRequestAutoScroll?: () => void;
};

type ExploreRowProps = {
  item: Extract<ConversationItem, { kind: "explore" }>;
};

const MessageImageGrid = memo(function MessageImageGrid({
  images,
  onOpen,
  hasText,
}: {
  images: MessageImage[];
  onOpen: (index: number) => void;
  hasText: boolean;
}) {
  return (
    <ul className={`message-image-grid${hasText ? " message-image-grid--with-text" : ""}`}>
      {images.map((image, index) => (
        <button
          key={image.src}
          type="button"
          className="message-image-thumb"
          onClick={() => onOpen(index)}
          aria-label={`Open image ${index + 1}`}
        >
          <img src={image.src} alt={image.label} loading="lazy" />
        </button>
      ))}
    </ul>
  );
});

const ImageLightbox = memo(function ImageLightbox({
  images,
  activeIndex,
  onClose,
}: {
  images: MessageImage[];
  activeIndex: number;
  onClose: () => void;
}) {
  const activeImage = images[activeIndex];

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  useEffect(() => {
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, []);

  if (!activeImage) {
    return null;
  }

  return createPortal(
    <div
      className="message-image-lightbox"
      role="dialog"
      aria-modal="true"
      onClick={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onClose();
        }
      }}
    >
      <div className="message-image-lightbox-content">
        <button
          type="button"
          className="message-image-lightbox-close"
          onClick={onClose}
          aria-label="Close image preview"
        >
          <X size={16} aria-hidden />
        </button>
        <img src={activeImage.src} alt={activeImage.label} />
      </div>
    </div>,
    document.body,
  );
});

export const WorkingIndicator = memo(function WorkingIndicator({
  isThinking,
  processingStartedAt = null,
  lastDurationMs = null,
  hasItems,
  reasoningLabel = null,
  showPollingFetchStatus = false,
  pollingIntervalMs = 12000,
}: WorkingIndicatorProps) {
  const [elapsedMs, setElapsedMs] = useState(0);
  const [pollCountdownSeconds, setPollCountdownSeconds] = useState(() =>
    Math.max(1, Math.ceil(pollingIntervalMs / 1000)),
  );

  useEffect(() => {
    if (!isThinking || !processingStartedAt) {
      setElapsedMs(0);
      return undefined;
    }
    setElapsedMs(Date.now() - processingStartedAt);
    const interval = window.setInterval(() => {
      setElapsedMs(Date.now() - processingStartedAt);
    }, 1000);
    return () => window.clearInterval(interval);
  }, [isThinking, processingStartedAt]);

  useEffect(() => {
    if (!showPollingFetchStatus || isThinking) {
      return undefined;
    }
    const intervalSeconds = Math.max(1, Math.ceil(pollingIntervalMs / 1000));
    setPollCountdownSeconds(intervalSeconds);
    const timer = window.setInterval(() => {
      setPollCountdownSeconds((previous) => (previous <= 1 ? intervalSeconds : previous - 1));
    }, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [isThinking, pollingIntervalMs, showPollingFetchStatus]);

  return (
    <>
      {isThinking && (
        <div className="working">
          <span className="working-spinner" aria-hidden />
          <div className="working-timer">
            <span className="working-timer-clock">{formatDurationMs(elapsedMs)}</span>
          </div>
          <span className="working-text">{reasoningLabel || "Working…"}</span>
        </div>
      )}
      {!isThinking && lastDurationMs !== null && hasItems && (
        <div className="turn-complete" aria-live="polite">
          <span className="turn-complete-line" aria-hidden />
          <span className="turn-complete-label">
            {showPollingFetchStatus
              ? `New message will be fetched in ${pollCountdownSeconds} seconds`
              : `Done in ${formatDurationMs(lastDurationMs)}`}
          </span>
          <span className="turn-complete-line" aria-hidden />
        </div>
      )}
    </>
  );
});

export const MessageRow = memo(function MessageRow({
  item,
  isCopied,
  onCopy,
  onQuote,
  codeBlockCopyUseModifier,
  showMessageFilePath,
  workspacePath,
  onOpenFileLink,
  onOpenFileLinkMenu,
  onOpenThreadLink,
}: MessageRowProps) {
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);
  const bubbleRef = useRef<HTMLDivElement | null>(null);
  const selectionSnapshotRef = useRef<string | null>(null);
  const hasText = item.text.trim().length > 0;
  const imageItems = useMemo(() => {
    if (!item.images || item.images.length === 0) {
      return [];
    }
    return item.images
      .map((image, index) => {
        const src = normalizeMessageImageSrc(image);
        if (!src) {
          return null;
        }
        return { src, label: `Image ${index + 1}` };
      })
      .filter(Boolean) as MessageImage[];
  }, [item.images]);
  const isTableOnlyAssistantMessage =
    item.role === "assistant" &&
    hasText &&
    imageItems.length === 0 &&
    isStandaloneMarkdownTable(item.text);

  const getSelectedMessageText = useCallback(() => {
    const bubble = bubbleRef.current;
    const selection = window.getSelection();
    if (!bubble || !selection || selection.rangeCount === 0 || selection.isCollapsed) {
      return null;
    }
    const selectedText = selection.toString().trim();
    if (!selectedText) {
      return null;
    }
    const range = selection.getRangeAt(0);
    if (!bubble.contains(range.commonAncestorContainer)) {
      return null;
    }

    const isWithinMessageControls = (node: Node | null) => {
      if (!node) {
        return false;
      }
      const element = node instanceof Element ? node : node.parentElement;
      return Boolean(element?.closest(".message-quote-button, .message-copy-button"));
    };

    if (
      isWithinMessageControls(selection.anchorNode) ||
      isWithinMessageControls(selection.focusNode)
    ) {
      return null;
    }
    return selectedText;
  }, []);

  const handleQuote = useCallback(() => {
    if (!onQuote) {
      return;
    }
    const selectedText = getSelectedMessageText() ?? selectionSnapshotRef.current ?? undefined;
    selectionSnapshotRef.current = null;
    onQuote(item, selectedText);
  }, [getSelectedMessageText, item, onQuote]);

  return (
    <div className={`message ${item.role}`}>
      <div
        ref={bubbleRef}
        className={`bubble message-bubble${isTableOnlyAssistantMessage ? " message-bubble-table-only" : ""}`}
      >
        {imageItems.length > 0 && (
          <MessageImageGrid images={imageItems} onOpen={setLightboxIndex} hasText={hasText} />
        )}
        {hasText && (
          <Markdown
            value={item.text}
            className="markdown"
            codeBlockStyle="message"
            codeBlockCopyUseModifier={codeBlockCopyUseModifier}
            showFilePath={showMessageFilePath}
            workspacePath={workspacePath}
            onOpenFileLink={onOpenFileLink}
            onOpenFileLinkMenu={onOpenFileLinkMenu}
            onOpenThreadLink={onOpenThreadLink}
          />
        )}
        {lightboxIndex !== null && imageItems.length > 0 && (
          <ImageLightbox
            images={imageItems}
            activeIndex={lightboxIndex}
            onClose={() => setLightboxIndex(null)}
          />
        )}
        {onQuote && hasText && (
          <button
            type="button"
            className="ghost message-quote-button"
            onMouseDown={() => {
              selectionSnapshotRef.current = getSelectedMessageText();
            }}
            onTouchStart={() => {
              selectionSnapshotRef.current = getSelectedMessageText();
            }}
            onClick={handleQuote}
            aria-label="Quote message"
            title="Quote message"
          >
            <Quote size={14} aria-hidden />
          </button>
        )}
        <button
          type="button"
          className={`ghost message-copy-button${isCopied ? " is-copied" : ""}`}
          onClick={() => onCopy(item)}
          aria-label="Copy message"
          title="Copy message"
        >
          <span className="message-copy-icon" aria-hidden>
            <Copy className="message-copy-icon-copy" size={14} />
            <Check className="message-copy-icon-check" size={14} />
          </span>
        </button>
      </div>
    </div>
  );
});

/// Inline thinking block — renders the model's chain-of-thought as plain
/// italicized prose with a `Thinking:` label, sitting in-stream right above
/// the tool call it narrates. No collapse, no truncation; the goal is the
/// transcript-style readability of Claude Code's terminal UI.
export const ReasoningRow = memo(function ReasoningRow({
  parsed,
  showMessageFilePath,
  workspacePath,
  onOpenFileLink,
  onOpenFileLinkMenu,
  onOpenThreadLink,
}: ReasoningRowProps) {
  const { bodyText, hasBody } = parsed;
  if (!hasBody) return null;
  // Approx token count via 4 chars/token heuristic. Cheap, stable, no extra
  // dependency. Mirrors kimi-cli's "Thought for Ns · N tokens" summary line.
  const approxTokens = Math.max(1, Math.round(bodyText.length / 4));
  return (
    <div className="reasoning-inline">
      <div className="reasoning-inline-meta">
        <span className="reasoning-inline-label">Thinking</span>
        <span className="reasoning-inline-meta-dot">·</span>
        <span className="reasoning-inline-meta-tokens">~{approxTokens} tokens</span>
      </div>
      <Markdown
        value={bodyText}
        className="reasoning-inline-body markdown"
        showFilePath={showMessageFilePath}
        workspacePath={workspacePath}
        onOpenFileLink={onOpenFileLink}
        onOpenFileLinkMenu={onOpenFileLinkMenu}
        onOpenThreadLink={onOpenThreadLink}
      />
    </div>
  );
});

export const ReviewRow = memo(function ReviewRow({
  item,
  showMessageFilePath,
  workspacePath,
  onOpenFileLink,
  onOpenFileLinkMenu,
  onOpenThreadLink,
}: ReviewRowProps) {
  const title = item.state === "started" ? "Review started" : "Review completed";
  return (
    <div className="item-card review">
      <div className="review-header">
        <span className="review-title">{title}</span>
        <span className={`review-badge ${item.state === "started" ? "active" : "done"}`}>
          Review
        </span>
      </div>
      {item.text && (
        <Markdown
          value={item.text}
          className="item-text markdown"
          showFilePath={showMessageFilePath}
          workspacePath={workspacePath}
          onOpenFileLink={onOpenFileLink}
          onOpenFileLinkMenu={onOpenFileLinkMenu}
          onOpenThreadLink={onOpenThreadLink}
        />
      )}
    </div>
  );
});

export const DiffRow = memo(function DiffRow({ item }: DiffRowProps) {
  return (
    <div className="item-card diff">
      <div className="diff-header">
        <span className="diff-title">{item.title}</span>
        {item.status && <span className="item-status">{item.status}</span>}
      </div>
      <div className="diff-viewer-output">
        <PierreDiffBlock diff={item.diff} displayPath={item.title} />
      </div>
    </div>
  );
});

export const UserInputRow = memo(function UserInputRow({
  item,
  isExpanded,
  onToggle,
}: UserInputRowProps) {
  const first = item.questions[0];
  const previewQuestion = first?.question?.trim() || first?.header?.trim() || "Input requested";
  const firstAnswer = first?.answers[0]?.trim() || "No answer provided";
  const previewAnswer =
    first && first.answers.length > 1 ? `${firstAnswer} +${first.answers.length - 1}` : firstAnswer;
  const extraQuestions = Math.max(0, item.questions.length - 1);

  return (
    <div className={`tool-inline user-input-inline ${isExpanded ? "tool-inline-expanded" : ""}`}>
      <button
        type="button"
        className="tool-inline-bar-toggle"
        onClick={() => onToggle(item.id)}
        aria-expanded={isExpanded}
        aria-label="Toggle answered input details"
      />
      <div className="tool-inline-content">
        <button
          type="button"
          className="tool-inline-summary tool-inline-toggle"
          onClick={() => onToggle(item.id)}
          aria-expanded={isExpanded}
        >
          <Check className="tool-inline-icon completed" size={14} aria-hidden />
          <span className="tool-inline-label">answered:</span>
          <span className="tool-inline-value user-input-inline-preview">
            {previewQuestion}: {previewAnswer}
            {extraQuestions > 0 ? ` +${extraQuestions} more` : ""}
          </span>
        </button>
        {isExpanded && (
          <div className="user-input-inline-details">
            {item.questions.map((question, index) => {
              const title = question.question || question.header || `Question ${index + 1}`;
              return (
                <div key={question.id} className="user-input-inline-entry">
                  <div className="user-input-inline-question">{title}</div>
                  {question.answers.length > 0 ? (
                    <div className="user-input-inline-answers">
                      {question.answers.map((answer, _answerIndex) => (
                        <div
                          key={`${question.id}-answer-${answer.slice(0, 32)}`}
                          className="user-input-inline-answer"
                        >
                          {answer}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="user-input-inline-empty-answer">No answer provided.</div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
});

const ASKUSER_TOOLTYPES = new Set(["mcpToolCall"]);

function isAskUser(item: Extract<ConversationItem, { kind: "tool" }>): boolean {
  if (!ASKUSER_TOOLTYPES.has(item.toolType)) return false;
  return /\bask_user\b/i.test(item.title);
}

export const ToolRow = memo(function ToolRow({
  item,
  isExpanded,
  onToggle,
  onRequestAutoScroll,
}: ToolRowProps) {
  const desc = toolRowDescriptor(item);
  const tone = toolStatusTone(item, (item.changes?.length ?? 0) > 0);
  const askUser = isAskUser(item);
  const isRunning = tone === "processing";
  const isFailed = tone === "failed";

  // Bash live tail — gated on existing 600ms warm-up + 1.2s long-running.
  const isCommand = item.toolType === "commandExecution";
  const durationMs = typeof item.durationMs === "number" ? item.durationMs : null;
  const isLongRunning = durationMs !== null && durationMs >= 1200;
  const [tailWarm, setTailWarm] = useState(false);
  useEffect(() => {
    if (!isRunning || !isCommand) {
      setTailWarm(false);
      return;
    }
    const handle = window.setTimeout(() => setTailWarm(true), 600);
    return () => window.clearTimeout(handle);
  }, [isCommand, isRunning]);

  const showTail =
    isCommand &&
    isRunning &&
    (item.output ?? "").length > 0 &&
    (tailWarm || isLongRunning) &&
    !isExpanded;

  useEffect(() => {
    if (showTail) onRequestAutoScroll?.();
  }, [showTail, onRequestAutoScroll]);

  const family = isFailed ? "failed" : desc.family;
  const className = [
    "tool-row",
    `tool-row--${family}`,
    isExpanded ? "is-expanded" : "",
    isRunning ? "is-running" : "",
    askUser ? "is-askuser" : "",
  ]
    .filter(Boolean)
    .join(" ");

  const handleClick = () => {
    if (askUser || isRunning) return;
    onToggle(item.id);
  };

  return (
    <>
      <div className={className}>
        <button
          type="button"
          className="tool-row__toggle"
          aria-label={`Toggle ${desc.name} details`}
          aria-expanded={isExpanded}
          disabled={askUser || isRunning}
          onClick={handleClick}
        >
          <span className="tool-row__icon" aria-hidden>
            {isRunning ? (
              <span className="tool-row__spinner" />
            ) : isFailed ? (
              <X size={11} aria-hidden />
            ) : null}
          </span>
          <span className="tool-row__name">{desc.name}:</span>
          {desc.arg && <span className="tool-row__arg">{desc.arg}</span>}
          {desc.meta.length > 0 && (
            <span className="tool-row__meta">
              {desc.meta.map((fragment, idx) => (
                <span key={`meta-${idx}`}>
                  {idx > 0 && <span className="tool-row__meta-sep">·</span>}
                  {fragment}
                </span>
              ))}
            </span>
          )}
          <span className="tool-row__chevron" aria-hidden>
            ▸
          </span>
        </button>
      </div>
      {showTail && <BashTail output={item.output ?? ""} />}
      {isExpanded && <ToolRowBody item={item} />}
    </>
  );
});

export const ExploreRow = memo(function ExploreRow({ item }: ExploreRowProps) {
  const isProcessing = item.status === "exploring";
  return (
    <div className={`tool-row tool-row--search${isProcessing ? " is-running" : ""}`}>
      <div className="tool-row__toggle" aria-disabled="true">
        <span className="tool-row__icon" aria-hidden>
          {isProcessing ? <span className="tool-row__spinner" /> : null}
        </span>
        <span className="tool-row__name">{isProcessing ? "Exploring" : "Explored"}:</span>
        <span className="tool-row__arg tool-row__explore-list">
          {item.entries.map((entry) => (
            <span
              key={`${entry.kind}-${entry.label}-${entry.detail ?? ""}`}
              className="tool-row__explore-item"
            >
              <span className="tool-row__explore-kind">{exploreKindLabel(entry.kind)}</span>
              <span className="tool-row__explore-label">{entry.label}</span>
            </span>
          ))}
        </span>
      </div>
    </div>
  );
});
