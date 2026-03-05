import { useMemo, useState } from 'react';
import { ChevronRight, Check, X } from 'lucide-react';
import { MarkdownContent } from './MarkdownContent';
import { formatDuration, formatTokens, qualifiedToolName } from '../../lib/utils';
import type { MessageSegment } from '../../lib/types';

interface SegmentedMessageProps {
  segments: MessageSegment[];
  /** Tool names currently executing (between ToolStart and ToolEnd). */
  activeTools?: string[];
  /** Whether this message is still streaming. */
  isStreaming?: boolean;
  /** The agent currently being delegated to (between delegation_started and delegation_completed). */
  activeDelegateAgent?: string | null;
}

// Rotate through accent colors for active tool spinners
const TOOL_COLORS = [
  { ring: 'border-brand/60', text: 'text-brand', dot: 'bg-brand' },         // orange
  { ring: 'border-info/60', text: 'text-info', dot: 'bg-info' },             // blue
  { ring: 'border-purple/60', text: 'text-purple', dot: 'bg-purple' },       // purple
  { ring: 'border-success/60', text: 'text-success', dot: 'bg-success' },    // green
] as const;

function toolColor(name: string) {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0;
  return TOOL_COLORS[Math.abs(hash) % TOOL_COLORS.length];
}

/** Try to pretty-print a JSON string; return as-is if not valid JSON. */
function formatResult(raw: string): string {
  const trimmed = raw.trimStart();
  if (trimmed[0] !== '{' && trimmed[0] !== '[') return raw;
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

type ToolSegment = Extract<MessageSegment, { type: 'tool' }>;

function CompletedToolSegment({ segment, nested }: { segment: ToolSegment; nested?: boolean }) {
  const isDelegate = segment.name === 'delegate';
  const [expanded, setExpanded] = useState(false);
  const qualifiedName = qualifiedToolName(segment.name, segment.action);
  const color = toolColor(qualifiedName);
  const formattedResult = useMemo(
    () => (segment.result && !isDelegate) ? formatResult(segment.result) : null,
    [segment.result, isDelegate],
  );
  const canExpand = !isDelegate && (formattedResult || segment.estimatedTokens);

  return (
    <div className={nested ? 'my-0.5' : 'my-1.5'}>
      <button
        onClick={() => canExpand && setExpanded(!expanded)}
        className={`flex items-center gap-1.5 text-[11px] font-light transition-colors ${color.text} ${canExpand ? 'hover:opacity-80' : 'cursor-default'}`}
      >
        {canExpand ? (
          <ChevronRight
            className={`w-3 h-3 transition-transform ${expanded ? 'rotate-90' : ''}`}
            strokeWidth={1.5}
          />
        ) : (
          <span className="w-3" />
        )}
        {segment.success ? (
          <Check className="w-3 h-3" strokeWidth={2} />
        ) : (
          <X className="w-3 h-3 text-destructive" strokeWidth={2} />
        )}
        <span>{qualifiedName}</span>
        <span className="text-dim">{formatDuration(segment.durationMs)}</span>
      </button>
      {expanded && (
        <div className="mt-1 ml-5 space-y-1">
          {formattedResult && (
            <pre className="p-2 text-[11px] font-light text-secondary bg-surface-base border border-border rounded-lg overflow-x-auto whitespace-pre-wrap break-words">
              {formattedResult}
            </pre>
          )}
          <div className="flex items-center gap-2 text-[10px] font-light text-dim">
            <span>{formatDuration(segment.durationMs)}</span>
            {segment.estimatedTokens && (
              <span>~{formatTokens(segment.estimatedTokens)} tokens</span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/** A completed delegate segment with its nested sub-agent tools. */
function DelegateGroup({ delegate, children }: { delegate: ToolSegment; children: ToolSegment[] }) {
  const [expanded, setExpanded] = useState(false);
  const color = toolColor('delegate');

  return (
    <div className="my-1.5">
      <button
        onClick={() => setExpanded(!expanded)}
        className={`flex items-center gap-1.5 text-[11px] font-light transition-colors ${color.text} hover:opacity-80`}
      >
        <ChevronRight
          className={`w-3 h-3 transition-transform ${expanded ? 'rotate-90' : ''}`}
          strokeWidth={1.5}
        />
        {delegate.success ? (
          <Check className="w-3 h-3" strokeWidth={2} />
        ) : (
          <X className="w-3 h-3 text-destructive" strokeWidth={2} />
        )}
        <span>delegate</span>
        <span className="text-dim">{formatDuration(delegate.durationMs)}</span>
        {children.length > 0 && (
          <span className="text-dim">({children.length} tool{children.length !== 1 ? 's' : ''})</span>
        )}
      </button>
      {expanded && (
        <div className="ml-5 border-l border-border/40 pl-2">
          {children.map((child, j) => (
            <CompletedToolSegment key={`sub-${j}-${child.name}`} segment={child} nested />
          ))}
        </div>
      )}
    </div>
  );
}

export function ActiveToolIndicator({ name, nested }: { name: string; nested?: boolean }) {
  const color = toolColor(name);

  return (
    <div className={`${nested ? 'my-0.5' : 'my-1.5'} flex items-center gap-1.5 text-[11px] font-light`}>
      <div className={`w-3 h-3 rounded-full border-[1.5px] ${color.ring} border-t-transparent animate-spin`} />
      <span className={color.text}>{name}</span>
      <span className="text-dim">&hellip;</span>
    </div>
  );
}

/**
 * Group completed segments: sub-agent tools (with `agent` set) that appear
 * BEFORE a `delegate` segment are grouped as children of that delegate.
 */
type RenderItem =
  | { kind: 'text'; index: number; segment: Extract<MessageSegment, { type: 'text' }> }
  | { kind: 'tool'; index: number; segment: ToolSegment }
  | { kind: 'delegate-group'; index: number; delegate: ToolSegment; children: ToolSegment[] };

function groupSegments(segments: MessageSegment[]): RenderItem[] {
  const items: RenderItem[] = [];
  // Accumulate agent-tagged tool segments; when we hit a delegate, consume them as children
  let pendingSubTools: ToolSegment[] = [];

  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    if (seg.type === 'text') {
      // Flush any pending sub-tools as standalone (no delegate followed them)
      for (const t of pendingSubTools) {
        items.push({ kind: 'tool', index: i, segment: t });
      }
      pendingSubTools = [];
      items.push({ kind: 'text', index: i, segment: seg });
    } else if (seg.name === 'delegate') {
      // This delegate owns the preceding agent-tagged tools
      items.push({ kind: 'delegate-group', index: i, delegate: seg, children: pendingSubTools });
      pendingSubTools = [];
    } else if (seg.agent) {
      // Sub-agent tool — hold it pending for the next delegate
      pendingSubTools.push(seg);
    } else {
      // Flush pending sub-tools, then add this regular tool
      for (const t of pendingSubTools) {
        items.push({ kind: 'tool', index: i, segment: t });
      }
      pendingSubTools = [];
      items.push({ kind: 'tool', index: i, segment: seg });
    }
  }
  // Trailing sub-tools (no delegate segment yet): flush as standalone items.
  // During streaming with an active delegate, the caller extracts these for nested display.
  for (const t of pendingSubTools) {
    items.push({ kind: 'tool', index: segments.length, segment: t });
  }
  return items;
}

export function SegmentedMessage({ segments, activeTools, isStreaming, activeDelegateAgent }: SegmentedMessageProps) {
  const lastIsText = segments.length > 0 && segments[segments.length - 1].type === 'text';
  const renderItems = useMemo(() => groupSegments(segments), [segments]);

  // During streaming, detect if delegate is the active parent tool
  const isDelegateActive = activeTools?.includes('delegate') && !!activeDelegateAgent;
  const subAgentActiveTools = isDelegateActive
    ? activeTools!.filter((t) => t !== 'delegate')
    : [];

  // During streaming with delegate active, completed sub-agent tool segments
  // (trailing items with agent set) should be shown nested under the delegate spinner
  const trailingSubTools: ToolSegment[] = [];
  let filteredItems = renderItems;
  if (isDelegateActive) {
    // Pull trailing agent-tagged tool items out — they'll render under delegate spinner
    filteredItems = [];
    // Walk backwards to find trailing sub-agent tools
    let foundNonSub = false;
    for (let i = renderItems.length - 1; i >= 0; i--) {
      const item = renderItems[i];
      if (!foundNonSub && item.kind === 'tool' && item.segment.agent) {
        trailingSubTools.unshift(item.segment);
      } else {
        foundNonSub = true;
        filteredItems.unshift(item);
      }
    }
  }

  return (
    <div>
      {filteredItems.map((item) => {
        if (item.kind === 'text') {
          const isLastText = item.index === segments.length - 1;
          return (
            <div key={`text-${item.index}`} className={isStreaming && isLastText ? 'streaming-cursor' : ''}>
              <MarkdownContent content={item.segment.content} />
            </div>
          );
        }
        if (item.kind === 'delegate-group') {
          return (
            <DelegateGroup
              key={`delegate-${item.index}`}
              delegate={item.delegate}
              children={item.children}
            />
          );
        }
        return <CompletedToolSegment key={`tool-${item.index}-${item.segment.name}`} segment={item.segment} />;
      })}

      {/* Active tool spinners — normal case (no delegate) */}
      {activeTools && !isDelegateActive && activeTools.map((name) => (
        <ActiveToolIndicator key={name} name={name} />
      ))}

      {/* Delegate active: show delegate spinner with completed + active sub-agent tools nested */}
      {isDelegateActive && (
        <div className="my-1.5">
          <ActiveToolIndicator name="delegate" />
          {(trailingSubTools.length > 0 || subAgentActiveTools.length > 0) && (
            <div className="ml-5 border-l border-border/40 pl-2">
              {trailingSubTools.map((seg, i) => (
                <CompletedToolSegment key={`sub-done-${i}-${seg.name}`} segment={seg} nested />
              ))}
              {subAgentActiveTools.map((name) => (
                <ActiveToolIndicator key={name} name={name} nested />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Cursor when streaming but last segment isn't text */}
      {isStreaming && !lastIsText && activeTools?.length === 0 && segments.length > 0 && (
        <span className="inline-block w-1.5 h-4 bg-muted/50 ml-0.5 animate-pulse align-text-bottom" />
      )}
    </div>
  );
}
