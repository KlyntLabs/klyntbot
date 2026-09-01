export interface NarrativeSnippet {
  id: string;
  headline: string;
  body: string;
  createdAt: string;
}

interface SnippetFeedProps {
  snippets: NarrativeSnippet[];
}

export function SnippetFeed({ snippets }: SnippetFeedProps) {
  if (snippets.length === 0) return null;

  return (
    <div className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground">Recent Insights</h2>
      {snippets.map((snippet) => (
        <div key={snippet.id} className="glass-panel rounded-xl p-4">
          <p className="text-[12px] font-medium text-foreground mb-1">{snippet.headline}</p>
          <p className="text-[11px] text-muted-foreground leading-relaxed">{snippet.body}</p>
        </div>
      ))}
    </div>
  );
}
