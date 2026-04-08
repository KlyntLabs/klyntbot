import { MemoryReference } from "@shared/ui";
import type { Components } from "react-markdown";
import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";

import { rehypeMemoryRef } from "../plugins/rehypeMemoryRef";

/** Strip internal confidence assessment tags from agent output. */
function stripConfidenceTags(text: string): string {
  return text.replace(/<confidence[^>]*\/?>(?:<\/confidence>)?/g, "").trimEnd();
}

const rehypePlugins = [rehypeHighlight, rehypeMemoryRef];
const remarkPlugins = [remarkGfm];

const customComponents: Partial<Components> = {
  // react-markdown passes HAST properties as camelCase React props directly
  // on the component (not nested under node.properties).
  "memory-ref": (props: Record<string, unknown>) => {
    // Props come directly as camelCase: dataRefType, dataRefId
    const refType = (props.dataRefType as string) ?? "";
    const refId = (props.dataRefId as string) ?? "";
    if (!refType || !refId) {
      // Fallback: check node.properties (older react-markdown versions)
      const el = props.node as { properties?: Record<string, string> } | undefined;
      const p = el?.properties ?? {};
      const fallbackType = p["data-ref-type"] ?? p.dataRefType ?? "";
      const fallbackId = p["data-ref-id"] ?? p.dataRefId ?? "";
      if (fallbackType && fallbackId) {
        return <MemoryReference refType={fallbackType} refId={fallbackId} />;
      }
      return null;
    }
    return <MemoryReference refType={refType} refId={refId} />;
  },
};

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export function MarkdownContent({ content, className = "" }: MarkdownContentProps) {
  const cleaned = stripConfidenceTags(content);

  return (
    <div className={`prose-content font-light ${className}`}>
      <Markdown
        remarkPlugins={remarkPlugins}
        rehypePlugins={rehypePlugins}
        components={customComponents}
      >
        {cleaned}
      </Markdown>
    </div>
  );
}
