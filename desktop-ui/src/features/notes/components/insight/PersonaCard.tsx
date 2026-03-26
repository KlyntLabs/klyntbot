import { MarkdownContent } from "@features/chat/components/MarkdownContent";
import { MessageCircle } from "lucide-react";
import { useState } from "react";
import { PersonaChat } from "./PersonaChat";

interface PersonaCardProps {
  name: string;
  role: string;
  icon: string;
  tone: string;
  content: string;
  noteId?: string;
  personaId?: string;
}

const TONE_COLORS: Record<string, { border: string; bg: string }> = {
  direct: { border: "border-l-red-400/60", bg: "bg-red-400/10" },
  skeptical: { border: "border-l-red-400/60", bg: "bg-red-400/10" },
  practical: { border: "border-l-amber-400/60", bg: "bg-amber-400/10" },
  pragmatic: { border: "border-l-amber-400/60", bg: "bg-amber-400/10" },
  curious: { border: "border-l-purple-400/60", bg: "bg-purple-400/10" },
  inquisitive: { border: "border-l-blue-400/60", bg: "bg-blue-400/10" },
  analytical: { border: "border-l-emerald-400/60", bg: "bg-emerald-400/10" },
  provocative: { border: "border-l-orange-400/60", bg: "bg-orange-400/10" },
  formal: { border: "border-l-gray-400/60", bg: "bg-gray-400/10" },
  neutral: { border: "border-l-gray-400/60", bg: "bg-gray-400/10" },
};

function getToneColor(tone: string) {
  return TONE_COLORS[tone] ?? { border: "border-l-gray-400/60", bg: "bg-gray-400/10" };
}

export function PersonaCard({
  name,
  role,
  icon,
  tone,
  content,
  noteId,
  personaId,
}: PersonaCardProps) {
  const colors = getToneColor(tone);
  const [showChat, setShowChat] = useState(false);

  return (
    <div className={`glass-card border-l-2 ${colors.border} rounded-lg p-3 space-y-2`}>
      {/* Header */}
      <div className="flex items-center gap-2">
        <span
          className={`size-7 rounded-full ${colors.bg} flex items-center justify-center text-sm shrink-0`}
        >
          {icon}
        </span>
        <div className="min-w-0">
          <div className="text-xs font-medium text-foreground truncate">{name}</div>
          <div className="text-2xs text-dim">{role}</div>
        </div>
      </div>

      {/* Analysis content */}
      <div className="text-xs text-muted-foreground leading-relaxed">
        <MarkdownContent content={content} />
      </div>

      {/* Inline chat toggle + chat */}
      {noteId && personaId && (
        <>
          <button
            type="button"
            onClick={() => setShowChat((p) => !p)}
            className="flex items-center gap-1 text-2xs text-purple hover:text-purple/80 transition-colors"
          >
            <MessageCircle size={10} />
            {showChat ? "Hide chat" : "Ask this persona"}
          </button>
          {showChat && <PersonaChat noteId={noteId} personaId={personaId} personaName={name} />}
        </>
      )}
    </div>
  );
}
