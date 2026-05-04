import AlertCircle from "lucide-react/dist/esm/icons/alert-circle";
import Check from "lucide-react/dist/esm/icons/check";
import Clock from "lucide-react/dist/esm/icons/clock";
import Copy from "lucide-react/dist/esm/icons/copy";
import X from "lucide-react/dist/esm/icons/x";
import { useEffect, useMemo, useRef, useState } from "react";
import { CausalGraphInspector } from "./CausalGraphInspector";
import { formatTimestamp } from "./eventHelpers";
import type { WireEventDto } from "./types";

interface ToolCallPair {
  toolCall: WireEventDto;
  toolResult: WireEventDto | null;
  toolName: string;
  toolCallId: string;
  durationMs: number | null;
  isError: boolean;
}

interface ToolCallDetailProps {
  selectedEvent: WireEventDto;
  allEvents: WireEventDto[];
  onClose: () => void;
}

function findPair(selected: WireEventDto, events: WireEventDto[]): ToolCallPair | null {
  let toolCallId: string | undefined;
  let toolCall: WireEventDto | undefined;
  let toolResult: WireEventDto | undefined;

  if (selected.kind === "toolCall") {
    toolCallId = (selected.payloadDecoded as Record<string, unknown> | undefined)?.id as
      | string
      | undefined;
    toolCall = selected;
  } else if (selected.kind === "toolResult") {
    toolCallId = (selected.payloadDecoded as Record<string, unknown> | undefined)?.tool_call_id as
      | string
      | undefined;
    toolResult = selected;
  }

  if (!toolCallId) return null;

  for (const e of events) {
    if (
      e.kind === "toolCall" &&
      (e.payloadDecoded as Record<string, unknown> | undefined)?.id === toolCallId &&
      !toolCall
    ) {
      toolCall = e;
    }
    if (
      e.kind === "toolResult" &&
      (e.payloadDecoded as Record<string, unknown> | undefined)?.tool_call_id === toolCallId &&
      !toolResult
    ) {
      toolResult = e;
    }
  }

  if (!toolCall) return null;

  const fn = (toolCall.payloadDecoded as Record<string, unknown> | undefined)?.function;
  const toolName = ((fn as { name?: unknown } | undefined)?.name as string) ?? "unknown";

  const durationMs =
    toolCall && toolResult
      ? new Date(toolResult.occurredAt).getTime() - new Date(toolCall.occurredAt).getTime()
      : null;

  const rv = toolResult?.payloadDecoded as Record<string, unknown> | undefined;
  const isError =
    ((rv?.return_value as Record<string, unknown> | undefined)?.is_error as boolean) === true;

  return {
    toolCall,
    toolResult: toolResult ?? null,
    toolName,
    toolCallId,
    durationMs,
    isError,
  };
}

function formatDuration(ms: number): string {
  if (ms < 1) return "<1ms";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}

export function ToolCallDetail({ selectedEvent, allEvents, onClose }: ToolCallDetailProps) {
  const pair = useMemo(() => findPair(selectedEvent, allEvents), [selectedEvent, allEvents]);
  if (!pair) return null;

  return (
    <div className="cm-tool-detail">
      <div className="cm-tool-detail__header">
        <span className="cm-tool-detail__name">{pair.toolName}</span>
        <span className="cm-tool-detail__id">{pair.toolCallId.slice(0, 16)}</span>
        {pair.durationMs != null && (
          <span className="cm-tool-detail__duration">
            <Clock size={10} />
            {formatDuration(pair.durationMs)}
          </span>
        )}
        {pair.isError && (
          <span className="cm-tool-detail__error-badge">
            <AlertCircle size={10} />
            Error
          </span>
        )}
        <CausalGraphInspector factIds={getRecallIds(pair.toolResult)} />
        <button type="button" className="cm-tool-detail__close" onClick={onClose}>
          <X size={14} />
        </button>
      </div>
      <div className="cm-tool-detail__body">
        <div className="cm-tool-detail__col">
          <div className="cm-tool-detail__col-title">
            ToolCall · {formatTimestamp(pair.toolCall.occurredAt)}
          </div>
          <CopyableJson data={getCallArgs(pair.toolCall)} />
        </div>
        <div className="cm-tool-detail__col">
          <div
            className={
              "cm-tool-detail__col-title" +
              (pair.isError ? " cm-tool-detail__col-title--error" : "")
            }
          >
            {pair.toolResult
              ? `ToolResult · ${formatTimestamp(pair.toolResult.occurredAt)}`
              : "ToolResult · (pending)"}
          </div>
          {pair.toolResult ? (
            <CopyableJson data={getResultOutput(pair.toolResult)} />
          ) : (
            <div className="cm-tool-detail__pending">No result yet</div>
          )}
        </div>
      </div>
    </div>
  );
}

function getCallArgs(event: WireEventDto): unknown {
  const fn = (event.payloadDecoded as Record<string, unknown> | undefined)?.function;
  if (!fn) return event.payloadDecoded;
  const argsStr = (fn as { arguments?: unknown }).arguments as string | undefined;
  if (argsStr) {
    try {
      return JSON.parse(argsStr);
    } catch {
      return argsStr;
    }
  }
  return fn;
}

function getResultOutput(event: WireEventDto): unknown {
  const p = event.payloadDecoded as Record<string, unknown> | undefined;
  if (p?.return_value !== undefined) return p.return_value;
  return p;
}

function getRecallIds(event: WireEventDto | null): string[] {
  if (!event) return [];
  const p = event.payloadDecoded as Record<string, unknown> | undefined;
  const ids = (p?.return_value as Record<string, unknown> | undefined)?.recall_ids;
  if (Array.isArray(ids)) return ids.filter((x): x is string => typeof x === "string");
  return [];
}

function CopyableJson({ data }: { data: unknown }) {
  const [copied, setCopied] = useState(false);
  const text = typeof data === "string" ? data : JSON.stringify(data, null, 2);
  const copiedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copiedTimeoutRef.current) {
        clearTimeout(copiedTimeoutRef.current);
      }
    };
  }, []);

  return (
    <div className="cm-tool-detail__json">
      <button
        type="button"
        className="cm-tool-detail__json-copy"
        onClick={async () => {
          await navigator.clipboard.writeText(text);
          setCopied(true);
          if (copiedTimeoutRef.current) {
            clearTimeout(copiedTimeoutRef.current);
          }
          copiedTimeoutRef.current = setTimeout(() => setCopied(false), 1500);
        }}
      >
        {copied ? <Check size={12} /> : <Copy size={12} />}
      </button>
      <pre>{text}</pre>
    </div>
  );
}
