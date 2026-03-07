import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";

/** Strip internal confidence assessment tags from agent output. */
function stripConfidenceTags(text: string): string {
  return text.replace(/<confidence[^>]*\/?>(?:<\/confidence>)?/g, "").trimEnd();
}

const rehypePlugins = [rehypeHighlight];
const remarkPlugins = [remarkGfm];

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export function MarkdownContent({ content, className = "" }: MarkdownContentProps) {
  const cleaned = stripConfidenceTags(content);

  return (
    <div className={`prose-content font-light ${className}`}>
      <Markdown remarkPlugins={remarkPlugins} rehypePlugins={rehypePlugins}>
        {cleaned}
      </Markdown>
    </div>
  );
}
