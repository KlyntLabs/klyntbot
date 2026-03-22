import * as ContextMenu from "@radix-ui/react-context-menu";
import { type ReactNode, useCallback, useRef, useState } from "react";

const TRANSLATE_LANGUAGES = [
  { code: "zh", label: "Chinese", native: "中文" },
  { code: "ja", label: "Japanese", native: "日本語" },
  { code: "ko", label: "Korean", native: "한국어" },
  { code: "vi", label: "Vietnamese", native: "Tiếng Việt" },
  { code: "en", label: "English", native: "English" },
  { code: "es", label: "Spanish", native: "Español" },
  { code: "fr", label: "French", native: "Français" },
  { code: "de", label: "German", native: "Deutsch" },
  { code: "ru", label: "Russian", native: "Русский" },
  { code: "ar", label: "Arabic", native: "العربية" },
  { code: "th", label: "Thai", native: "ไทย" },
  { code: "hi", label: "Hindi", native: "हिन्दी" },
  { code: "pt", label: "Portuguese", native: "Português" },
];

interface EditorContextMenuProps {
  children: ReactNode;
  onAnnotate: () => void;
  onFlashcard: () => void;
  onTranslate: (selectedText: string, rect?: { top: number; left: number }) => void;
  onTranslateTo: (
    targetLang: string,
    selectedText?: string,
    rect?: { top: number; left: number },
  ) => void;
  onAskAI: () => void;
  onRemoveAnnotation?: (annotationId: string) => void;
  noteTargetLang?: string;
}

export function EditorContextMenu({
  children,
  onAnnotate,
  onFlashcard,
  onTranslate,
  onTranslateTo,
  onAskAI,
  onRemoveAnnotation,
  noteTargetLang,
}: EditorContextMenuProps) {
  const [hadSelection, setHadSelection] = useState(false);
  const [annotationId, setAnnotationId] = useState<string | null>(null);
  const selectionTextRef = useRef("");
  const savedRangeRef = useRef<Range | null>(null);
  const selectionRectRef = useRef<{ top: number; left: number } | undefined>(undefined);

  const handleOpenChange = useCallback((open: boolean) => {
    if (open) {
      const sel = window.getSelection();
      const text = sel?.toString().trim() ?? "";
      setHadSelection(text.length > 0);
      selectionTextRef.current = text;
      // Capture rect now — it will be gone after the menu closes
      if (sel && sel.rangeCount > 0) {
        const rect = sel.getRangeAt(0).getBoundingClientRect();
        selectionRectRef.current = { top: rect.bottom + 8, left: rect.left };
      } else {
        selectionRectRef.current = undefined;
      }
      // Detect if right-click is on an annotation
      const focusNode = sel?.focusNode;
      const el = focusNode instanceof HTMLElement ? focusNode : focusNode?.parentElement;
      const annEl = el?.closest(".annotation-highlight") as HTMLElement | null;
      setAnnotationId(annEl?.getAttribute("data-annotation-id") ?? null);

      // Save range for CSS Highlight API (non-destructive visual highlight)
      if (text.length > 0 && sel && sel.rangeCount > 0) {
        savedRangeRef.current = sel.getRangeAt(0).cloneRange();
        try {
          // @ts-expect-error CSS Highlight API — Chromium 105+
          const highlight = new Highlight(savedRangeRef.current);
          // @ts-expect-error CSS Highlight API
          CSS.highlights?.set("editor-context-selection", highlight);
        } catch {
          // Fallback: no visual highlight on older browsers
        }
      }
    } else {
      setAnnotationId(null);
      savedRangeRef.current = null;
      try {
        // @ts-expect-error CSS Highlight API
        CSS.highlights?.delete("editor-context-selection");
      } catch {
        // ignore
      }
    }
  }, []);

  return (
    <ContextMenu.Root onOpenChange={handleOpenChange}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content
          className="glass-panel min-w-[200px] rounded-lg p-1.5 shadow-xl"
          onOpenAutoFocus={(e) => e.preventDefault()}
          onCloseAutoFocus={(e) => e.preventDefault()}
          onFocusOutside={(e) => e.preventDefault()}
        >
          {hadSelection && (
            <>
              <ContextMenu.Label className="px-2 py-1 text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
                Selection
              </ContextMenu.Label>
              <MenuItem onClick={onAnnotate} shortcut="⌥A">
                Annotate
              </MenuItem>
              <MenuItem onClick={onFlashcard} shortcut="⌥F">
                Create Flashcard
              </MenuItem>
              {/* Translate → language picker submenu */}
              <ContextMenu.Sub>
                <ContextMenu.SubTrigger className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover">
                  Translate
                  <span className="text-muted ml-4">▸</span>
                </ContextMenu.SubTrigger>
                <ContextMenu.Portal>
                  <ContextMenu.SubContent className="glass-panel min-w-[180px] max-h-[320px] overflow-y-auto rounded-lg p-1.5 shadow-xl">
                    {TRANSLATE_LANGUAGES.map((lang) => (
                      <ContextMenu.Item
                        key={lang.code}
                        onClick={() =>
                          onTranslateTo(
                            lang.code,
                            selectionTextRef.current,
                            selectionRectRef.current,
                          )
                        }
                        className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover"
                      >
                        <span>
                          {lang.native} <span className="text-muted">({lang.label})</span>
                        </span>
                        {noteTargetLang === lang.code && (
                          <span className="text-brand text-[10px]">●</span>
                        )}
                      </ContextMenu.Item>
                    ))}
                  </ContextMenu.SubContent>
                </ContextMenu.Portal>
              </ContextMenu.Sub>
              <ContextMenu.Separator className="my-1 h-px bg-border" />
            </>
          )}

          {/* Remove annotation — only when right-clicking on annotated text */}
          {annotationId && onRemoveAnnotation && (
            <>
              <MenuItem onClick={() => onRemoveAnnotation(annotationId)}>
                <span className="text-red-400">Remove annotation</span>
              </MenuItem>
              <ContextMenu.Separator className="my-1 h-px bg-border" />
            </>
          )}

          <ContextMenu.Label className="px-2 py-1 text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
            AI Actions
          </ContextMenu.Label>
          <MenuItem onClick={onAskAI}>Ask AI</MenuItem>
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function MenuItem({
  onClick,
  shortcut,
  children,
}: {
  onClick: () => void;
  shortcut?: string;
  children: ReactNode;
}) {
  return (
    <ContextMenu.Item
      onClick={onClick}
      className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover"
    >
      <span>{children}</span>
      {shortcut && <span className="ml-4 text-muted text-[10px]">{shortcut}</span>}
    </ContextMenu.Item>
  );
}
