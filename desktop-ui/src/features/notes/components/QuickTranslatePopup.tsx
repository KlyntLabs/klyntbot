import { useCopyToClipboard } from "@shared/hooks/useCopyToClipboard";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Check, Copy } from "lucide-react";
import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

interface QuickTranslatePopupProps {
  translation: string | null;
  position: { top: number; left: number };
  loading?: boolean;
  onDismiss: () => void;
}

export function QuickTranslatePopup({
  translation,
  position,
  loading,
  onDismiss,
}: QuickTranslatePopupProps) {
  const popupRef = useRef<HTMLDivElement>(null);
  const hasData = !!translation;

  useEffect(() => {
    function handleMouseDown(e: MouseEvent) {
      if (popupRef.current && !popupRef.current.contains(e.target as Node)) {
        onDismiss();
      }
    }
    document.addEventListener("mousedown", handleMouseDown);
    return () => document.removeEventListener("mousedown", handleMouseDown);
  }, [onDismiss]);

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onDismiss();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onDismiss]);

  return createPortal(
    <div
      ref={popupRef}
      className="fixed z-50 max-w-[320px] min-w-[200px] rounded-xl p-3 shadow-2xl backdrop-blur-[24px] backdrop-saturate-[1.6]"
      style={{
        top: position.top,
        left: position.left,
        background: "var(--surface-floating)",
        border: "1px solid var(--glass-border)",
      }}
    >
      {/* Loading state */}
      {loading && !hasData && (
        <div className="flex items-center gap-2 py-1">
          <ThinkingDots size="sm" />
          <span className="text-ui-sm text-fg-secondary">Translating...</span>
        </div>
      )}

      {/* Translation text + copy button */}
      {hasData && <TranslationResult text={translation} />}
    </div>,
    document.body,
  );
}

function TranslationResult({ text }: { text: string }) {
  const { copied, copy } = useCopyToClipboard(1500);

  return (
    <div className="flex items-start gap-2">
      <p className="text-sm text-brand leading-relaxed flex-1">{text}</p>
      <button
        type="button"
        onClick={() => copy(text)}
        className="shrink-0 mt-0.5 p-1 rounded text-fg-secondary hover:text-brand transition-colors"
        title="Copy translation"
      >
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </div>
  );
}
