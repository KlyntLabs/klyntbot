import ReactMarkdown from "react-markdown";

export function SkillMarkdown({ content }: { content: string }) {
  const body = stripFrontmatter(content);
  return (
    <div className="prose prose-invert max-w-none text-sm">
      <ReactMarkdown>{body}</ReactMarkdown>
    </div>
  );
}

function stripFrontmatter(s: string): string {
  const m = s.match(/^---[\s\S]*?---\n?/);
  return m ? s.slice(m[0].length) : s;
}
