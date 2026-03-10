import { ChevronDown, FolderOpen, Mic, Plus, Send, Server, Shield } from "lucide-react";
import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";

interface ChatInputProps {
  input: string;
  isStreaming: boolean;
  onInputChange: (value: string) => void;
  onSend: () => void;
}

export function ChatInput({ input, isStreaming, onInputChange, onSend }: ChatInputProps) {
  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);

  return (
    <div className="p-6">
      <div className="max-w-3xl mx-auto">
        <div className="glass-input flex items-center px-2 gap-2">
          <button
            type="button"
            aria-label="Add attachment"
            className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0 rounded-lg hover:bg-white/[0.05]"
          >
            <Plus className="w-4 h-4" strokeWidth={1.5} />
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
            className="flex-1 bg-transparent py-3.5 text-[13px] text-primary placeholder:text-muted font-light resize-none overflow-hidden outline-none"
            style={{ maxHeight: "200px" }}
          />
          <button
            type="button"
            aria-label="Voice input"
            className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0 rounded-lg hover:bg-white/[0.05]"
          >
            <Mic className="w-4 h-4" strokeWidth={1.5} />
          </button>
          <button
            type="button"
            onClick={onSend}
            disabled={!input.trim() || isStreaming}
            aria-label="Send message"
            className="w-9 h-9 rounded-full bg-brand hover:bg-brand-hover disabled:bg-white/[0.06] disabled:text-muted flex items-center justify-center transition-all shrink-0"
          >
            <Send className="w-4 h-4" strokeWidth={2} />
          </button>
        </div>
        <div className="flex items-center justify-between mt-2">
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="glass-button flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light text-muted"
            >
              <Server className="w-3.5 h-3.5" strokeWidth={1.5} />
              <span>Local</span>
              <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
            </button>
            <button
              type="button"
              className="glass-button flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light text-muted"
            >
              <Shield className="w-3.5 h-3.5" strokeWidth={1.5} />
              <span>Default permissions</span>
              <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
            </button>
          </div>
          <button
            type="button"
            className="glass-button flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light text-muted"
          >
            <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
            <span>KlyntBot</span>
            <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
          </button>
        </div>
      </div>
    </div>
  );
}
