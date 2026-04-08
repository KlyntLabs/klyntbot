import { MessageSquareMore } from "lucide-react";

interface CollapsedInteractionProps {
  content: string;
}

export function CollapsedInteraction({ content }: CollapsedInteractionProps) {
  return (
    <div className="flex justify-start">
      <div className="flex items-center gap-2 px-4 py-2 glass-card rounded-xl">
        <MessageSquareMore className="size-3.5 text-brand shrink-0" strokeWidth={1.5} />
        <span className="text-xs font-light text-muted-foreground">{content}</span>
      </div>
    </div>
  );
}
