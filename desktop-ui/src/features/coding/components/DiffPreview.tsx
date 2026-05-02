import { PierreDiffBlock } from "@/features/git/components/PierreDiffBlock";
import type { ConversationItem } from "@/types";

type DiffItem = Extract<ConversationItem, { kind: "diff" }>;

export function DiffPreview({ item }: { item: DiffItem }) {
  return (
    <div className="diff-preview">
      <header className="diff-preview__header">
        <span className="diff-preview__path">{item.path}</span>
        {item.op && (
          <span className={`diff-preview__op diff-preview__op--${item.op}`}>{item.op}</span>
        )}
        {typeof item.bytes === "number" && (
          <span className="diff-preview__bytes">{item.bytes} bytes</span>
        )}
      </header>
      <PierreDiffBlock diff={item.diff} displayPath={item.path ?? item.title} />
    </div>
  );
}
