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
  // react-markdown renders unknown HTML elements via their tagName
  "memory-ref": (props: React.ComponentProps<"span"> & { node?: unknown }) => {
    const el = props.node as { properties?: Record<string, string> } | undefined;
    const refType = el?.properties?.["data-ref-type"] ?? "";
    const refId = el?.properties?.["data-ref-id"] ?? "";
    if (!refType || !refId) return null;
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
