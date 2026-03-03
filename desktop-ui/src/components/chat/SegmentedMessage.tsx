import { useState } from 'react';
import { ChevronRight, Check, X } from 'lucide-react';
import { MarkdownContent } from './MarkdownContent';
import { formatDuration } from '../../lib/utils';
import type { MessageSegment } from '../../lib/types';

interface SegmentedMessageProps {
  segments: MessageSegment[];
  /** Tool names currently executing (between ToolStart and ToolEnd). */
  activeTools?: string[];
  /** Whether this message is still streaming. */
  isStreaming?: boolean;
}

// Rotate through accent colors for active tool spinners
const TOOL_COLORS = [
  { ring: 'border-brand/60', text: 'text-brand', dot: 'bg-brand' },         // orange
  { ring: 'border-info/60', text: 'text-info', dot: 'bg-info' },             // blue
  { ring: 'border-purple/60', text: 'text-purple', dot: 'bg-purple' },       // purple
  { ring: 'border-success/60', text: 'text-success', dot: 'bg-success' },    // green
] as const;

function toolColor(name: string) {
  // Simple hash to get a stable color per tool name
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0;
  return TOOL_COLORS[Math.abs(hash) % TOOL_COLORS.length];
}

function CompletedToolSegment({ segment }: { segment: Extract<MessageSegment, { type: 'tool' }> }) {
  const [expanded, setExpanded] = useState(false);
  const color = toolColor(segment.name);

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
        {segment.success ? (
          <Check className="w-3 h-3" strokeWidth={2} />
        ) : (
          <X className="w-3 h-3 text-destructive" strokeWidth={2} />
        )}
        <span>{segment.name}</span>
        <span className="text-dim">{formatDuration(segment.durationMs)}</span>
      </button>
      {expanded && segment.result && (
        <pre className="mt-1 ml-5 p-2 text-[11px] font-light text-secondary bg-surface-base border border-border rounded-lg overflow-x-auto whitespace-pre-wrap break-words">
          {segment.result}
        </pre>
      )}
    </div>
  );
}

function ActiveToolIndicator({ name }: { name: string }) {
  const color = toolColor(name);

  return (
    <div className="my-1.5 flex items-center gap-1.5 text-[11px] font-light">
      <div className={`w-3 h-3 rounded-full border-[1.5px] ${color.ring} border-t-transparent animate-spin`} />
      <span className={color.text}>{name}</span>
      <span className="text-dim">&hellip;</span>
    </div>
  );
}

export function SegmentedMessage({ segments, activeTools, isStreaming }: SegmentedMessageProps) {
  const lastIsText = segments.length > 0 && segments[segments.length - 1].type === 'text';

  return (
    <div>
      {segments.map((seg, i) => {
        const isLastText = seg.type === 'text' && i === segments.length - 1;
        // Use index + name as key for tools (index guarantees uniqueness,
        // name adds semantic stability). Text segments use type-prefixed index
        // since only the last one mutates during streaming.
        const key = seg.type === 'tool' ? `tool-${i}-${seg.name}` : `text-${i}`;
        return seg.type === 'text' ? (
          <div key={key} className={isStreaming && isLastText ? 'streaming-cursor' : ''}>
            <MarkdownContent content={seg.content} />
          </div>
        ) : (
          <CompletedToolSegment key={key} segment={seg} />
        );
      })}

      {/* Active tool spinners (between ToolStart and ToolEnd) */}
      {activeTools && activeTools.map((name) => (
        <ActiveToolIndicator key={name} name={name} />
      ))}

      {/* Cursor when streaming but last segment isn't text (e.g., after a tool completes) */}
      {isStreaming && !lastIsText && activeTools?.length === 0 && segments.length > 0 && (
        <span className="inline-block w-1.5 h-4 bg-muted/50 ml-0.5 animate-pulse align-text-bottom" />
      )}
    </div>
  );
}
