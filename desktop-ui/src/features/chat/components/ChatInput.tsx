import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";
import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { cn } from "@shared/lib/utils";
import { Mic, Plus, Send } from "lucide-react";
import { useState } from "react";

interface ChatInputProps {
  input: string;
  isStreaming: boolean;
  onInputChange: (value: string) => void;
  onSend: () => void;
}

export function ChatInput({ input, isStreaming, onInputChange, onSend }: ChatInputProps) {
  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);
  const [isDictating, setIsDictating] = useState(false);

  const startDictation = async () => {
    try {
      setIsDictating(true);
      await ipc("voice_start_dictation");
    } catch {
      setIsDictating(false);
    }
  };

  const stopDictation = async () => {
    if (!isDictating) return;
    setIsDictating(false); // Clear immediately to block re-entrant calls
    try {
      const transcript = await ipc<string>("voice_stop_dictation");
      if (transcript) {
        onInputChange(input ? `${input} ${transcript}` : transcript);
      }
    } catch {
      // stop_capture returns Ok(None) gracefully when session is already gone
    }
  };

  useEvent<Record<string, unknown>>("voice:event", (payload) => {
    if (isDictating && payload.type === "captureEnded") {
      stopDictation();
    }
  });

  return (
    <div className="p-6">
      <div className="max-w-3xl mx-auto">
        <div className="glass-input flex items-center px-2 gap-2">
          <button
            type="button"
            aria-label="Add attachment"
            className="size-8 flex items-center justify-center text-fg-secondary hover:text-fg transition-colors shrink-0 rounded-lg hover:bg-control-hover"
          >
            <Plus className="size-4" strokeWidth={1.5} />
          </button>
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => onInputChange(e.target.value)}
            onInput={handleInput}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                onSend();
              }
            }}
            aria-label="Message input"
            placeholder="Ask Klynt anything, @ to add files, / for commands"
            rows={1}
            className="flex-1 bg-transparent py-3.5 text-ui text-fg placeholder:text-fg-secondary font-light resize-none overflow-hidden outline-none"
            style={{ maxHeight: "200px" }}
          />
          <button
            type="button"
            aria-label={isDictating ? "Stop dictation" : "Voice input"}
            onClick={isDictating ? stopDictation : startDictation}
            className={cn(
              "size-8 flex items-center justify-center transition-colors shrink-0 rounded-lg",
              isDictating
                ? "text-status-danger animate-pulse bg-status-danger/10"
                : "text-fg-secondary hover:text-fg hover:bg-control-hover",
            )}
          >
            <Mic className="size-4" strokeWidth={1.5} />
          </button>
          <button
            type="button"
            onClick={onSend}
            disabled={!input.trim() || isStreaming}
            aria-label="Send message"
            className="size-9 rounded-full bg-brand text-brand-foreground hover:bg-brand-hover disabled:bg-control-hover disabled:text-fg-secondary flex items-center justify-center transition-all shrink-0"
          >
            <Send className="size-4" strokeWidth={2} />
          </button>
        </div>
      </div>
    </div>
  );
}
