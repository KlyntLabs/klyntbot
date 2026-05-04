import { Markdown } from "@/features/messages/components/Markdown";

export function TextPart({ text }: { text: string }) {
  if (!text) return null;
  return (
    <div className="part-text">
      <Markdown value={text} />
    </div>
  );
}
