import { useAutoResizeTextarea } from "@shared/hooks/useAutoResizeTextarea";
import { useClickOutside } from "@shared/hooks/useClickOutside";
import { ChevronDown, GitMerge, Mic, Plus, Send, Users } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useSquads } from "../../notes/hooks/useSquads";
import type { VoiceMode } from "./VoiceToggle";

interface ChatInputProps {
  input: string;
  isStreaming: boolean;
  squadId: string | null;
  voiceMode: VoiceMode;
  onInputChange: (value: string) => void;
  onSend: () => void;
  onSelectSquad: (squadId: string) => void;
  onSelectDefault: () => void;
  onVoiceModeChange: (mode: VoiceMode) => void;
}

export function ChatInput({
  input,
  isStreaming,
  squadId,
  voiceMode,
  onInputChange,
  onSend,
  onSelectSquad,
  onSelectDefault,
  onVoiceModeChange,
}: ChatInputProps) {
  const { ref: textareaRef, handleInput } = useAutoResizeTextarea(input);
  const { squads } = useSquads();
  const [popup, setPopup] = useState<{ x: number; y: number } | null>(null);
  const popupRef = useRef<HTMLDivElement>(null);
  useClickOutside(popupRef, () => setPopup(null), !!popup);

  // Close on Escape
  useEffect(() => {
    if (!popup) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPopup(null);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [popup]);

  const currentSquad = squads.find((s) => s.id === squadId);

  const handleModeClick = (e: React.MouseEvent) => {
    if (popup) {
      setPopup(null);
    } else {
      setPopup({ x: e.clientX, y: e.clientY });
    }
  };

  const handleSquadSelect = (id: string) => {
    setPopup(null);
    if (id !== squadId) onSelectSquad(id);
  };

  const handleDefaultSelect = () => {
    setPopup(null);
    if (squadId) onSelectDefault();
  };

  return (
    <div className="p-6">
      <div className="max-w-3xl mx-auto">
        <div className="glass-input flex items-center px-2 gap-2">
          <button
            type="button"
            aria-label="Add attachment"
            className="size-8 flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors shrink-0 rounded-lg hover:bg-accent"
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
            className="flex-1 bg-transparent py-3.5 text-[13px] text-foreground placeholder:text-muted-foreground font-light resize-none overflow-hidden outline-none"
            style={{ maxHeight: "200px" }}
          />
          <button
            type="button"
            aria-label="Voice input"
            className="size-8 flex items-center justify-center text-muted-foreground hover:text-foreground transition-colors shrink-0 rounded-lg hover:bg-accent"
          >
            <Mic className="size-4" strokeWidth={1.5} />
          </button>
          <button
            type="button"
            onClick={onSend}
            disabled={!input.trim() || isStreaming}
            aria-label="Send message"
            className="size-9 rounded-full bg-brand hover:bg-brand-hover disabled:bg-accent disabled:text-muted-foreground flex items-center justify-center transition-all shrink-0"
          >
            <Send className="size-4" strokeWidth={2} />
          </button>
        </div>
        <div className="flex items-center gap-2 mt-2">
          {/* Squad mode picker */}
          <button
            type="button"
            onClick={handleModeClick}
            className={`glass-button flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-light ${
              currentSquad ? "text-purple-400" : "text-muted-foreground"
            }`}
          >
            {currentSquad ? (
              <>
                <span className="text-xs leading-none">{currentSquad.icon}</span>
                <span className="truncate max-w-[100px]">{currentSquad.name}</span>
              </>
            ) : (
              <>
                <Users className="size-3.5" strokeWidth={1.5} />
                <span>KlyntBot</span>
              </>
            )}
            <ChevronDown
              className={`size-3 transition-transform ${popup ? "rotate-180" : ""}`}
              strokeWidth={1.5}
            />
          </button>
          {/* Voice mode toggle — only when squad is active */}
          {squadId && (
            <div className="flex items-center gap-0.5 rounded-lg bg-white/[0.04] p-0.5">
              <button
                type="button"
                onClick={() => onVoiceModeChange("multi")}
                className={`flex items-center gap-1 text-2xs px-2 py-1 rounded-md transition-colors ${
                  voiceMode === "multi"
                    ? "bg-purple-500/20 text-purple-300"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                <Users size={10} />
                Multi
              </button>
              <button
                type="button"
                onClick={() => onVoiceModeChange("synthesized")}
                className={`flex items-center gap-1 text-2xs px-2 py-1 rounded-md transition-colors ${
                  voiceMode === "synthesized"
                    ? "bg-purple-500/20 text-purple-300"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                <GitMerge size={10} />
                Merged
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Squad mode popup — portal, opens upward from click */}
      {popup &&
        createPortal(
          <div
            ref={popupRef}
            role="menu"
            onMouseDown={(e) => e.stopPropagation()}
            className="fixed z-50 glass-dropdown rounded-xl py-1.5 px-1.5 animate-[menu-appear_120ms_ease-out]"
            style={{
              left: Math.max(8, Math.min(popup.x - 100, window.innerWidth - 240)),
              bottom: window.innerHeight - popup.y + 4,
            }}
          >
            {/* Squad options — compact single-line rows */}
            {squads.map((squad) => (
              <button
                key={squad.id}
                type="button"
                role="menuitem"
                onClick={() => handleSquadSelect(squad.id)}
                className={`flex items-center gap-2 w-full px-2.5 py-[5px] rounded-md text-left transition-colors ${
                  squad.id === squadId
                    ? "bg-purple-500/20 text-purple-300"
                    : "text-muted-foreground hover:bg-white/[0.06] hover:text-foreground"
                }`}
              >
                <span className="text-xs leading-none shrink-0">{squad.icon}</span>
                <span className="text-[11px] font-medium truncate flex-1">{squad.name}</span>
                {/* Inline member avatars */}
                <div className="flex items-center -space-x-1 shrink-0">
                  {squad.members.slice(0, 3).map((m) => (
                    <span
                      key={m.personaId}
                      className="size-3.5 rounded-full bg-white/[0.08] flex items-center justify-center text-[7px] ring-1 ring-black/30"
                      title={m.personaName}
                    >
                      {m.personaIcon}
                    </span>
                  ))}
                  {squad.members.length > 3 && (
                    <span className="text-[8px] text-dim ml-1.5">+{squad.members.length - 3}</span>
                  )}
                </div>
              </button>
            ))}

            <div className="h-px bg-white/[0.08] my-1 mx-1.5" />

            {/* Default KlyntBot option */}
            <button
              type="button"
              role="menuitem"
              onClick={handleDefaultSelect}
              className={`flex items-center gap-2 w-full px-2.5 py-[5px] rounded-md text-left transition-colors ${
                !squadId
                  ? "bg-brand/15 text-foreground"
                  : "text-muted-foreground hover:bg-white/[0.06] hover:text-foreground"
              }`}
            >
              <Users className="size-3 shrink-0" strokeWidth={1.5} />
              <span className="text-[11px] font-medium">KlyntBot</span>
            </button>
          </div>,
          document.body,
        )}
    </div>
  );
}
