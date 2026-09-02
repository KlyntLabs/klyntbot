interface PersonaMessageProps {
  personaName: string;
  personaIcon?: string;
  personaRole?: string;
  content: string;
}

export function PersonaMessage({
  personaName,
  personaIcon,
  personaRole,
  content,
}: PersonaMessageProps) {
  return (
    <div className="flex gap-2 py-2">
      <div className="shrink-0 size-7 rounded-full bg-purple/10 flex items-center justify-center text-sm">
        {personaIcon || "🤖"}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-1.5 mb-1">
          <span className="text-ui-xs font-medium text-fg">{personaName}</span>
          {personaRole && <span className="text-[9px] text-fg-dim">{personaRole}</span>}
        </div>
        <div className="text-ui-sm text-fg-secondary leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    </div>
  );
}
