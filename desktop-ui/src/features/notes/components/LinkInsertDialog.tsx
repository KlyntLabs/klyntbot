import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

interface LinkInsertDialogProps {
  type: "link" | "image";
  isOpen: boolean;
  onClose: () => void;
  onInsert: (url: string) => void;
}

export function LinkInsertDialog({ type, isOpen, onClose, onInsert }: LinkInsertDialogProps) {
  const [url, setUrl] = useState("");
  const [previewError, setPreviewError] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-focus and reset on open
  useEffect(() => {
    if (isOpen) {
      setUrl("");
      setPreviewError(false);
      // Defer focus to next frame so the portal has rendered
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [isOpen]);

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [isOpen, onClose]);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = url.trim();
      if (trimmed) {
        onInsert(trimmed);
        onClose();
      }
    },
    [url, onInsert, onClose],
  );

  if (!isOpen) return null;

  return createPortal(
    // biome-ignore lint/a11y/noStaticElementInteractions: dialog backdrop overlay
    <div
      className="fixed inset-0 z-[200] flex items-center justify-center"
      role="presentation"
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-overlay-heavy" />

      {/* Dialog */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation on dialog panel */}
      <div
        className="relative glass-panel rounded-2xl p-5 w-[380px] shadow-2xl"
        role="dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="text-sm font-semibold text-foreground mb-3">
          {type === "image" ? "Insert Image" : "Insert Link"}
        </h3>

        <form onSubmit={handleSubmit}>
          <label className="block text-xs text-muted-foreground mb-1.5" htmlFor="link-url-input">
            {type === "image" ? "Image URL" : "URL"}
          </label>
          <input
            ref={inputRef}
            id="link-url-input"
            type="url"
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              setPreviewError(false);
            }}
            placeholder={type === "image" ? "https://example.com/image.png" : "https://example.com"}
            className="w-full px-3 py-2 text-sm rounded-lg bg-accent border border-border text-foreground placeholder:text-dim outline-none focus:border-brand/40 transition-colors"
          />

          {/* Image preview */}
          {type === "image" && url.trim() && (
            <div className="mt-3 rounded-lg overflow-hidden bg-card flex items-center justify-center min-h-[80px] max-h-[160px]">
              {!previewError ? (
                <img
                  src={url.trim()}
                  alt="Preview"
                  className="max-w-full max-h-[160px] object-contain"
                  onError={() => setPreviewError(true)}
                />
              ) : (
                <div className="text-xs text-dim py-4">Unable to preview image</div>
              )}
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center justify-end gap-2 mt-4">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-xs rounded-lg text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!url.trim()}
              className="px-3 py-1.5 text-xs rounded-lg bg-brand/20 text-brand hover:bg-brand/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Insert
            </button>
          </div>
        </form>
      </div>
    </div>,
    document.body,
  );
}
