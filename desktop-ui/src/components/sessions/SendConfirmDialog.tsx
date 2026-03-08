import { Send, X } from "lucide-react";
import { sessionStatusColor, type TrackedSession } from "../../lib/session-types";

interface SendConfirmDialogProps {
  open: boolean;
  session: TrackedSession;
  content: string;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
}

export function SendConfirmDialog({
  open,
  session,
  content,
  onClose,
  onConfirm,
}: SendConfirmDialogProps) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="glass-panel w-[440px]">
        <div className="bg-white/[0.04] rounded-[var(--glass-radius-inner)]">
          {/* Header */}
          <div className="flex items-center justify-between px-5 py-4 border-b border-white/[0.08]">
            <h3 className="text-[14px] font-medium text-primary">Send to Claude Code</h3>
            <button
              type="button"
              onClick={onClose}
              className="w-7 h-7 rounded-md flex items-center justify-center text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Body */}
          <div className="px-5 py-4 space-y-3">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <span className="text-[12px] text-muted font-light">Session:</span>
                <span className="text-[12px] text-primary font-light truncate">
                  {session.projectName}
                </span>
              </div>
              {session.gitBranch && (
                <div className="flex items-center gap-2">
                  <span className="text-[12px] text-muted font-light">Branch:</span>
                  <span className="text-[12px] text-primary font-light">{session.gitBranch}</span>
                </div>
              )}
              <div className="flex items-center gap-2">
                <span className="text-[12px] text-muted font-light">Status:</span>
                <span className={`text-[12px] font-light ${sessionStatusColor(session.status)}`}>
                  {session.status}
                </span>
              </div>
            </div>

            <div className="glass-divider" />

            <div>
              <span className="text-[11px] text-dim font-light">Message:</span>
              <pre className="mt-1 text-[12px] text-primary font-light whitespace-pre-wrap break-words max-h-[200px] overflow-y-auto">
                {content}
              </pre>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-white/[0.08]">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg text-[12px] font-light text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={onConfirm}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] font-light bg-brand hover:bg-brand-hover text-white transition-colors"
            >
              <Send className="w-3 h-3" strokeWidth={2} />
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
