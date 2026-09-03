import { formatHumanDuration } from "@shared/lib/dates";
import type { WorkContext } from "@shared/types";
import { Search, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useSearchWorkContexts } from "../hooks/useWorkContexts";
import { contextColor } from "../lib/context-colors";

interface ContextSearchDialogProps {
  open: boolean;
  onClose: () => void;
  onSelect: (ctx: WorkContext) => void;
}

export function ContextSearchDialog({ open, onClose, onSelect }: ContextSearchDialogProps) {
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const { data: results } = useSearchWorkContexts(debouncedQuery);

  // Debounce search input
  useEffect(() => {
    if (!query.trim()) {
      setDebouncedQuery(null);
      return;
    }
    const timer = setTimeout(() => setDebouncedQuery(query.trim()), 300);
    return () => clearTimeout(timer);
  }, [query]);

  // Focus input on open
  useEffect(() => {
    if (open) {
      setQuery("");
      setDebouncedQuery(null);
      const timer = setTimeout(() => inputRef.current?.focus(), 50);
      return () => clearTimeout(timer);
    }
  }, [open]);

  // Escape to close
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: backdrop overlay, Escape handled via document listener */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop overlay */}
      <div className="fixed inset-0 z-50 bg-overlay" onClick={onClose} />
      <div className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh] pointer-events-none">
        <div className="glass-panel w-full max-w-lg pointer-events-auto rounded-2xl overflow-hidden">
          {/* Search input */}
          <div className="flex items-center gap-3 px-4 py-3 border-b border-separator">
            <Search className="size-4 text-fg-secondary shrink-0" />
            <input
              ref={inputRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search work contexts…"
              className="flex-1 bg-transparent text-sm text-fg placeholder-muted outline-none"
            />
            <button type="button" onClick={onClose} className="text-fg-secondary hover:text-fg">
              <X className="size-4" />
            </button>
          </div>

          {/* Results */}
          <div className="max-h-80 overflow-y-auto py-2">
            {debouncedQuery && results.length === 0 && (
              <p className="text-ui-sm text-fg-secondary px-4 py-3">No contexts found</p>
            )}
            {results.map((ctx) => {
              const color = contextColor(ctx.color, ctx.contextType);
              return (
                <button
                  key={ctx.id}
                  type="button"
                  onClick={() => {
                    onSelect(ctx);
                    onClose();
                  }}
                  className="w-full flex items-center gap-3 px-4 py-2.5 hover:bg-control-hover transition-colors text-left"
                >
                  <div
                    className="size-2.5 rounded-full shrink-0"
                    style={{ backgroundColor: color }}
                  />
                  <div className="flex-1 min-w-0">
                    <p className="text-ui text-fg font-medium truncate">{ctx.title}</p>
                    <p className="text-ui-xs text-fg-secondary">
                      {ctx.contextType} · {formatHumanDuration(ctx.totalDurationSecs)} ·{" "}
                      {ctx.eventCount} events
                    </p>
                  </div>
                  <span className="text-ui-xs text-fg-secondary shrink-0">{ctx.status}</span>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </>,
    document.body,
  );
}
