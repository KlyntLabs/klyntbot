import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Clipboard, FileText, MessageSquare } from "lucide-react";
import { useState } from "react";
import { NotePicker } from "./NotePicker";

type QuickGenMode = null | "note" | "clipboard";

interface QuickGenerateProps {
  onGenerateFromNote: (noteId: string) => void;
  onGenerateFromText: (text: string) => void;
  generating: boolean;
}

export function QuickGenerate({
  onGenerateFromNote,
  onGenerateFromText,
  generating,
}: QuickGenerateProps) {
  const [mode, setMode] = useState<QuickGenMode>(null);
  const [clipboardText, setClipboardText] = useState("");

  if (generating) {
    return (
      <div className="glass-card p-4 flex items-center justify-center gap-2">
        <ThinkingDots size="sm" />
        <span className="text-sm text-muted-foreground">Generating cards</span>
      </div>
    );
  }

  if (mode === "note") {
    return (
      <div className="glass-card p-4">
        <p className="text-xs text-muted-foreground mb-2">Select a note to generate from:</p>
        <NotePicker
          onSelect={(note) => {
            setMode(null);
            onGenerateFromNote(note.id);
          }}
          onCancel={() => setMode(null)}
        />
      </div>
    );
  }

  if (mode === "clipboard") {
    return (
      <div className="glass-card p-4 space-y-2">
        <p className="text-xs text-muted-foreground">Paste text to generate flashcards:</p>
        <textarea
          value={clipboardText}
          onChange={(e) => setClipboardText(e.target.value)}
          placeholder="Paste content here..."
          className="w-full bg-muted/50 rounded-lg px-3 py-2 text-sm text-foreground placeholder:text-dim resize-none"
          rows={4}
        />
        <div className="flex items-center justify-between">
          <button
            type="button"
            onClick={() => setMode(null)}
            className="text-xs text-muted-foreground hover:text-foreground"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => {
              if (clipboardText.trim()) {
                setMode(null);
                onGenerateFromText(clipboardText);
              }
            }}
            disabled={!clipboardText.trim()}
            className="glass-button px-3 py-1.5 text-xs text-foreground disabled:opacity-40"
          >
            Generate
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 text-left">
      <p className="text-sm font-medium text-foreground mb-3">Quick Generate</p>
      <div className="space-y-1.5">
        <button
          type="button"
          onClick={() => setMode("note")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-left"
        >
          <FileText size={14} strokeWidth={1.5} />
          From note...
        </button>
        <button
          type="button"
          onClick={() => setMode("clipboard")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-left"
        >
          <Clipboard size={14} strokeWidth={1.5} />
          From clipboard...
        </button>
        <button
          type="button"
          disabled
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-muted-foreground opacity-50 cursor-not-allowed text-left"
        >
          <MessageSquare size={14} strokeWidth={1.5} />
          From last chat...
          <span className="ml-auto text-2xs">Soon</span>
        </button>
      </div>
    </div>
  );
}
