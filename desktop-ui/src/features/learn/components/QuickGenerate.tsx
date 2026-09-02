import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { ThinkingDots } from "@shared/ui/ThinkingDots";
import { Clipboard, FileText, MessageSquare } from "lucide-react";
import { useCallback, useState } from "react";
import { NotePicker } from "./NotePicker";

type QuickGenMode = null | "note" | "clipboard" | "conversations";

function ConversationPicker({
  onSelect,
  onCancel,
}: {
  onSelect: (text: string) => void;
  onCancel: () => void;
}) {
  const { data: sessions, loading } = useQuery<
    { sessionKey: string; title: string; updatedAt: string; preview: string }[]
  >("flashcard_recent_learning_sessions", { limit: 3 }, []);
  const [selecting, setSelecting] = useState(false);

  const handleSelect = useCallback(
    async (sessionKey: string) => {
      setSelecting(true);
      try {
        const messages = await ipc<{ role: string; content: string }[]>("chat_messages", {
          sessionKey,
          limit: 50,
        });
        const text = messages
          .filter((m) => m.role === "user" || m.role === "assistant")
          .map((m) => m.content)
          .join("\n\n");
        onSelect(text);
      } catch {
        setSelecting(false);
      }
    },
    [onSelect],
  );

  if (loading || selecting) {
    return (
      <div className="glass-card p-4 text-center">
        <p className="text-ui-sm text-fg-secondary">
          {selecting ? "Loading conversation..." : "Loading conversations..."}
        </p>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="glass-card p-4 space-y-2">
        <p className="text-ui-sm text-fg-secondary">No recent conversations found.</p>
        <button
          type="button"
          onClick={onCancel}
          className="text-ui-sm text-fg-secondary hover:text-fg"
        >
          Back
        </button>
      </div>
    );
  }

  return (
    <div className="glass-card p-4 space-y-2">
      <p className="text-ui-sm text-fg-secondary mb-2">Select a conversation:</p>
      {sessions.map((s) => (
        <button
          key={s.sessionKey}
          type="button"
          onClick={() => handleSelect(s.sessionKey)}
          className="w-full text-left px-3 py-2 rounded-lg hover:bg-control-hover transition-colors"
        >
          <p className="text-sm font-medium text-fg truncate">{s.title}</p>
          <p className="text-ui-xs text-fg-secondary truncate mt-0.5">{s.preview}</p>
        </button>
      ))}
      <button
        type="button"
        onClick={onCancel}
        className="text-ui-sm text-fg-secondary hover:text-fg mt-1"
      >
        Cancel
      </button>
    </div>
  );
}

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

  const handleSelectConversation = useCallback(
    (text: string) => {
      setMode(null);
      onGenerateFromText(text);
    },
    [onGenerateFromText],
  );

  if (generating) {
    return (
      <div className="glass-card p-4 flex items-center justify-center gap-2">
        <ThinkingDots size="sm" />
        <span className="text-sm text-fg-secondary">Generating cards</span>
      </div>
    );
  }

  if (mode === "note") {
    return (
      <div className="glass-card p-4">
        <p className="text-ui-sm text-fg-secondary mb-2">Select a note to generate from:</p>
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
        <p className="text-ui-sm text-fg-secondary">Paste text to generate flashcards:</p>
        <textarea
          value={clipboardText}
          onChange={(e) => setClipboardText(e.target.value)}
          placeholder="Paste content here..."
          className="w-full bg-control-hover/50 rounded-lg px-3 py-2 text-sm text-fg placeholder:text-fg-dim resize-none"
          rows={4}
        />
        <div className="flex items-center justify-between">
          <button
            type="button"
            onClick={() => setMode(null)}
            className="text-ui-sm text-fg-secondary hover:text-fg"
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
            className="glass-button px-3 py-1.5 text-ui-sm text-fg disabled:opacity-40"
          >
            Generate
          </button>
        </div>
      </div>
    );
  }

  if (mode === "conversations") {
    return (
      <ConversationPicker onSelect={handleSelectConversation} onCancel={() => setMode(null)} />
    );
  }

  return (
    <div className="glass-card p-4 text-left">
      <p className="text-sm font-medium text-fg mb-3">Quick Generate</p>
      <div className="space-y-1.5">
        <button
          type="button"
          onClick={() => setMode("note")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors text-left"
        >
          <FileText size={14} strokeWidth={1.5} />
          From note...
        </button>
        <button
          type="button"
          onClick={() => setMode("clipboard")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors text-left"
        >
          <Clipboard size={14} strokeWidth={1.5} />
          From clipboard...
        </button>
        <button
          type="button"
          onClick={() => setMode("conversations")}
          className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm text-fg-secondary hover:text-fg hover:bg-control-hover transition-colors text-left"
        >
          <MessageSquare size={14} strokeWidth={1.5} />
          From recent conversations...
        </button>
      </div>
    </div>
  );
}
