import type { Components } from "react-markdown";
import Markdown from "react-markdown";

/** Strip internal confidence assessment tags from agent output. */
function stripConfidenceTags(text: string): string {
  return text.replace(/<confidence[^>]*\/?>(?:<\/confidence>)?/g, "").trimEnd();
}

const components: Components = {
  // Headings
  h1: ({ children }) => (
    <h1 className="text-[15px] font-medium text-primary mt-4 mb-2 first:mt-0">{children}</h1>
  ),
  h2: ({ children }) => (
    <h2 className="text-[14px] font-medium text-primary mt-3.5 mb-1.5 first:mt-0">{children}</h2>
  ),
  h3: ({ children }) => (
    <h3 className="text-[13px] font-medium text-primary mt-3 mb-1 first:mt-0">{children}</h3>
  ),

  // Paragraph
  p: ({ children }) => <p className="mb-2.5 last:mb-0 leading-relaxed">{children}</p>,

  // Bold & italic
  strong: ({ children }) => <strong className="font-medium text-primary">{children}</strong>,
  em: ({ children }) => <em className="italic text-secondary">{children}</em>,

  // Inline code
  code: ({ children, className }) => {
    // Fenced code blocks get a `language-*` className from react-markdown
    if (className) {
      return <code className="block text-[12px] font-mono leading-relaxed">{children}</code>;
    }
    return (
      <code className="text-[12px] font-mono bg-white/[0.08] px-1.5 py-0.5 rounded-md text-brand">
        {children}
      </code>
    );
  },

  // Code block
  pre: ({ children }) => (
    <pre className="bg-white/[0.08] rounded-lg px-4 py-3 my-2.5 overflow-x-auto border border-white/[0.08]/50 text-secondary">
      {children}
    </pre>
  ),

  // Lists
  ul: ({ children }) => (
    <ul className="list-disc list-outside pl-5 mb-2.5 last:mb-0 space-y-1">{children}</ul>
  ),
  ol: ({ children }) => (
    <ol className="list-decimal list-outside pl-5 mb-2.5 last:mb-0 space-y-1">{children}</ol>
  ),
  li: ({ children }) => <li className="leading-relaxed">{children}</li>,

  // Links
  a: ({ children, href }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-brand hover:text-brand/80 underline underline-offset-2"
    >
      {children}
    </a>
  ),

  // Blockquote
  blockquote: ({ children }) => (
    <blockquote className="border-l-2 border-muted/40 pl-3.5 my-2.5 text-muted italic">
      {children}
    </blockquote>
  ),

  // Horizontal rule
  hr: () => <hr className="border-white/[0.08]/50 my-4" />,

  // Table
  table: ({ children }) => (
    <div className="overflow-x-auto my-2.5">
      <table className="w-full text-[12px]">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="border-b border-white/[0.08]/50">{children}</thead>,
  th: ({ children }) => (
    <th className="text-left font-medium text-primary px-2 py-1.5">{children}</th>
  ),
  td: ({ children }) => <td className="px-2 py-1.5 border-b border-white/[0.08]/30">{children}</td>,
};

interface MarkdownContentProps {
  content: string;
  className?: string;
}

export function MarkdownContent({ content, className = "" }: MarkdownContentProps) {
  const cleaned = stripConfidenceTags(content);

  return (
    <div className={`text-[13px] font-light text-secondary ${className}`}>
      <Markdown components={components}>{cleaned}</Markdown>
    </div>
  );
}
