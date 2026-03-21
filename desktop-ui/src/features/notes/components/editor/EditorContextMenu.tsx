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
  onTranslate: () => void;
  onTranslateTo: (targetLang: string, selectedText?: string) => void;
  onAskAI: () => void;
  onLinkedView: () => void;
  onApplyPerspective: (type: string) => void;
  noteTargetLang?: string;
}

export function EditorContextMenu({
  children,
  onAnnotate,
  onFlashcard,
  onTranslate,
  onTranslateTo,
  onAskAI,
  onLinkedView,
  onApplyPerspective,
  noteTargetLang,
}: EditorContextMenuProps) {
  const [hadSelection, setHadSelection] = useState(false);
  const selectionTextRef = useRef("");

  const handleOpenChange = useCallback((open: boolean) => {
    if (open) {
      const sel = window.getSelection();
      const text = sel?.toString().trim() ?? "";
      setHadSelection(text.length > 0);
      selectionTextRef.current = text;
    }
  }, []);

  return (
    <ContextMenu.Root onOpenChange={handleOpenChange}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="glass-panel min-w-[200px] rounded-lg p-1.5 shadow-xl">
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
              <MenuItem onClick={onTranslate}>Translate</MenuItem>
              <ContextMenu.Separator className="my-1 h-px bg-border" />
            </>
          )}

          <ContextMenu.Label className="px-2 py-1 text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
            AI Actions
          </ContextMenu.Label>
          <MenuItem onClick={onAskAI}>Ask AI</MenuItem>
          <MenuItem onClick={onLinkedView} shortcut="⌥L">
            Linked View
          </MenuItem>

          {/* Translate to... submenu */}
          <ContextMenu.Sub>
            <ContextMenu.SubTrigger className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover">
              {hadSelection ? "Translate selection to…" : "Translate to…"}
              <span className="text-muted ml-4">▸</span>
            </ContextMenu.SubTrigger>
            <ContextMenu.Portal>
              <ContextMenu.SubContent className="glass-panel min-w-[180px] max-h-[320px] overflow-y-auto rounded-lg p-1.5 shadow-xl">
                {TRANSLATE_LANGUAGES.map((lang) => (
                  <ContextMenu.Item
                    key={lang.code}
                    onClick={() =>
                      onTranslateTo(lang.code, hadSelection ? selectionTextRef.current : undefined)
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

          <ContextMenu.Sub>
            <ContextMenu.SubTrigger className="flex items-center justify-between rounded-md px-2 py-1.5 text-xs text-primary outline-none select-none data-[highlighted]:bg-surface-hover">
              Apply Perspective
              <span className="text-muted ml-4">▸</span>
            </ContextMenu.SubTrigger>
            <ContextMenu.Portal>
              <ContextMenu.SubContent className="glass-panel min-w-[160px] rounded-lg p-1.5 shadow-xl">
                <MenuItem onClick={() => onApplyPerspective("linked-view")}>Linked View</MenuItem>
                <MenuItem onClick={() => onApplyPerspective("annotated")}>Annotated</MenuItem>
                <MenuItem onClick={() => onApplyPerspective("study-mode")}>Study Mode</MenuItem>
              </ContextMenu.SubContent>
            </ContextMenu.Portal>
          </ContextMenu.Sub>
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
