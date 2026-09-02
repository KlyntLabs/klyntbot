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
    <div className="island rounded-panel p-3 space-y-1">
      <span className="text-2xs text-fg-secondary uppercase tracking-wider">Related Knowledge</span>
      {matchingTopics.map((topic) => (
        <div key={topic.id} className="flex items-center justify-between text-ui-sm">
          <span className="text-fg truncate">{topic.name}</span>
          <span className={`text-2xs tabular-nums ${retentionTextColor(topic.avgRetention)}`}>
            {Math.round(topic.avgRetention * 100)}%
          </span>
        </div>
      ))}
    </div>
  );
}
