import { useKnowledgeHealth } from "@features/learn/hooks/useKnowledgeHealth";
import { retentionTextColor } from "@shared/lib/retention";

interface RelevantAtomsProps {
  domain?: string;
  limit?: number;
}

export function RelevantAtoms({ domain, limit = 5 }: RelevantAtomsProps) {
  const { data } = useKnowledgeHealth();

  const matchingTopics = domain
    ? data.topics.filter((t) => t.domain.toLowerCase().includes(domain.toLowerCase()))
    : data.topics.slice(0, limit);

  if (matchingTopics.length === 0) return null;

  return (
    <div className="glass-card rounded-lg p-3 space-y-1">
      <span className="text-[10px] text-muted uppercase tracking-wider">Related Knowledge</span>
      {matchingTopics.map((topic) => (
        <div key={topic.id} className="flex items-center justify-between text-xs">
          <span className="text-primary truncate">{topic.name}</span>
          <span className={`text-[10px] tabular-nums ${retentionTextColor(topic.avgRetention)}`}>
            {Math.round(topic.avgRetention * 100)}%
          </span>
        </div>
      ))}
    </div>
  );
}
