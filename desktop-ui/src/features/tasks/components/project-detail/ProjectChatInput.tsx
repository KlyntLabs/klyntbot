import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";
import { Send } from "lucide-react";
import { useState } from "react";

interface ProjectChatInputProps {
  projectId: string;
}

export function ProjectChatInput({ projectId: _projectId }: ProjectChatInputProps) {
  const [input, setInput] = useState("");
  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);

  const handleSend = () => {
    if (!input.trim()) return;
    // TODO: Wire to chat_send with project context
    setInput("");
  };

  return (
    <div className="px-4 py-3 border-t border-white/[0.06] shrink-0">
      <div className="flex items-center gap-2">
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onInput={handleInput}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleSend();
            }
          }}
          placeholder="Ask about this project..."
          rows={1}
          className="flex-1 bg-white/[0.04] rounded-lg px-3 py-2.5 text-[13px] text-primary placeholder:text-muted font-light resize-none overflow-hidden outline-none border border-white/[0.06] focus:border-brand/40 transition-colors"
          style={{ maxHeight: "120px" }}
        />
        <button
          type="button"
          onClick={handleSend}
          disabled={!input.trim()}
          className="w-8 h-8 rounded-full bg-brand hover:bg-brand-hover disabled:bg-white/[0.06] disabled:text-muted flex items-center justify-center transition-all shrink-0"
        >
          <Send className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
      </div>
    </div>
  );
}
