import { useMutation } from "@shared/hooks/useMutation";
import { Send } from "lucide-react";
import { useRef, useState } from "react";

interface MirrorResponse {
  answer: string;
  dataSourcesUsed: string[];
}

export function MirrorInput() {
  const [query, setQuery] = useState("");
  const [response, setResponse] = useState<MirrorResponse | null>(null);
  const { mutate: generateResponse, loading } = useMutation<
    MirrorResponse,
    Record<string, unknown>
  >("generate_mirror_response");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSubmit = async () => {
    const trimmed = query.trim();
    if (!trimmed || loading) return;
    setResponse(null);
    const result = await generateResponse({ query: trimmed });
    if (result) {
      setResponse(result);
      setQuery("");
      if (textareaRef.current) textareaRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setQuery(e.target.value);
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${textareaRef.current.scrollHeight}px`;
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="island rounded-xl p-4">
        <div className="flex items-end gap-2">
          <textarea
            ref={textareaRef}
            value={query}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            placeholder="Ask me about how I think..."
            rows={1}
            className="flex-1 bg-transparent text-ui-sm text-fg placeholder:text-fg-dim resize-none outline-none leading-relaxed max-h-32 overflow-y-auto"
          />
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!query.trim() || loading}
            className="shrink-0 p-1.5 rounded-lg text-fg-secondary hover:text-brand hover:bg-brand/10 transition-colors disabled:opacity-40 disabled:pointer-events-none"
            aria-label="Send"
          >
            <Send className="size-4" />
          </button>
        </div>
      </div>

      {loading && (
        <div className="glass-panel rounded-xl p-4">
          <p className="text-ui-xs text-fg-secondary animate-pulse">Thinking...</p>
        </div>
      )}

      {response && !loading && (
        <div className="glass-panel rounded-xl p-4">
          <p className="text-ui-sm text-fg leading-relaxed">{response.answer}</p>
          {response.dataSourcesUsed.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-3">
              {response.dataSourcesUsed.map((src) => (
                <span
                  key={src}
                  className="text-ui-xs px-1.5 py-0.5 rounded bg-control-hover/40 text-fg-secondary"
                >
                  {src}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
