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
  // rehypeMemoryRef converts [@type:id] markers into <memory-ref> elements,
  // stripping them from the visible text. We render nothing — markers are
  // for backend tracking (retrieval feedback), not user-facing display.
  "memory-ref": () => null,
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
