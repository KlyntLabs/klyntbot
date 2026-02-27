import { motion } from 'motion/react';
import { Terminal, Wifi, WifiOff } from 'lucide-react';
import type { ChatMessage } from '../../../lib/types';
import type { ConnectionStatus } from '../../../lib/ws';
import { renderMarkdown, formatTime } from '../utils';

/** Connection status indicator component */
export function StatusDot({ status }: { status: ConnectionStatus }) {
  const color =
    status === 'connected'
      ? '#10b981'
      : status === 'connecting' || status === 'reconnecting'
        ? '#f59e0b'
        : '#ef4444';

  const Icon =
    status === 'connected' || status === 'connecting' || status === 'reconnecting'
      ? Wifi
      : WifiOff;

  return (
    <div className="flex items-center gap-1.5" title={`WebSocket: ${status}`}>
      <Icon className="w-3 h-3" strokeWidth={1.5} style={{ color }} />
      <span
        className="text-[10px] uppercase tracking-wide"
        style={{ color, fontFamily: 'var(--font-mono)' }}
      >
        {status}
      </span>
    </div>
  );
}

export function MessageBubble({ msg }: { msg: ChatMessage }) {
  return (
    <motion.div
      key={msg.id}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex flex-col"
    >
      {msg.role === 'user' && (
        <div className="flex justify-end">
          <div
            className="max-w-[85%] px-4 py-3 rounded-lg"
            style={{
              backgroundColor: 'var(--codex-bg-user)',
              color: 'var(--codex-fg)',
            }}
          >
            <div
              className="text-[14px] leading-relaxed whitespace-pre-wrap"
              style={{ color: 'var(--codex-fg)' }}
            >
              {msg.content}
            </div>
            <div
              className="text-[11px] mt-2"
              style={{
                color: 'var(--codex-fg-subtle)',
                fontFamily: 'var(--font-mono)',
              }}
            >
              {formatTime(msg.timestamp)}
            </div>
          </div>
        </div>
      )}

      {msg.role === 'assistant' && (
        <div className="flex gap-3">
          <div className="flex-1">
            <div
              className="text-[14px] leading-relaxed prose-chat"
              style={{ color: 'var(--codex-fg)' }}
              dangerouslySetInnerHTML={{ __html: renderMarkdown(msg.content) }}
            />
            {msg.isStreaming && (
              <span
                className="inline-block w-[2px] h-[14px] ml-0.5 align-text-bottom animate-pulse"
                style={{ backgroundColor: 'var(--codex-accent)' }}
              />
            )}
            <div className="flex items-center gap-2 mt-2">
              <div
                className="text-[11px]"
                style={{
                  color: 'var(--codex-fg-subtle)',
                  fontFamily: 'var(--font-mono)',
                }}
              >
                {formatTime(msg.timestamp)}
              </div>
            </div>
          </div>
        </div>
      )}

      {msg.role === 'system' && (
        <div className="flex justify-center py-2">
          <div
            className="flex items-center gap-2 px-3 py-1.5 rounded-md text-[12px]"
            style={{
              backgroundColor: 'var(--codex-bg-secondary)',
              border: '1px solid var(--codex-border)',
              color: 'var(--codex-fg-subtle)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            <Terminal className="w-3 h-3" strokeWidth={1.5} />
            {msg.content}
          </div>
        </div>
      )}
    </motion.div>
  );
}
