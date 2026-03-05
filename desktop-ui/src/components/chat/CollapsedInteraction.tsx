import { MessageSquareMore } from "lucide-react";

interface CollapsedInteractionProps {
  content: string;
}

export function CollapsedInteraction({ content }: CollapsedInteractionProps) {
  return (
    <div className="flex justify-start">
      <div className="flex items-center gap-2 px-4 py-2 rounded-xl bg-surface-base border border-border">
        <MessageSquareMore className="w-3.5 h-3.5 text-brand shrink-0" strokeWidth={1.5} />
        <span className="text-[12px] font-light text-secondary">{content}</span>
      </div>
    </div>
  );
}
