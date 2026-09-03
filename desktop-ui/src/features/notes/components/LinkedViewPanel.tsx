import { useLinkedContext } from "../hooks/useLinkedContext";

interface LinkedViewPanelProps {
  noteId: string;
  sectionText: string;
}

export function LinkedViewPanel({ noteId, sectionText }: LinkedViewPanelProps) {
  const { context, loading } = useLinkedContext(noteId, sectionText);

  const totalLinks =
    context.semanticFacts.length +
    context.episodicMemories.length +
    context.relatedAnnotations.length +
    context.proceduralRules.length;

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-ui-sm text-fg-secondary">
        Loading cognitive links...
      </div>
    );
  }

  if (totalLinks === 0) {
    return (
      <div className="flex h-full items-center justify-center text-ui-sm text-fg-secondary">
        No cognitive links found for this section.
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto p-4">
      <h3 className="text-ui-sm font-medium text-fg-secondary">
        {totalLinks} cognitive link{totalLinks !== 1 ? "s" : ""}
      </h3>

      {/* Semantic Facts */}
      {context.semanticFacts.length > 0 && (
        <Section title="Semantic Facts" color="purple">
          {context.semanticFacts.map((fact) => (
            <div
              key={fact.id}
              className="rounded-md border border-purple-500/20 bg-purple-500/5 p-2"
            >
              <p className="text-ui-sm text-brand">
                {fact.subject} <span className="text-fg-secondary">{fact.predicate}</span>{" "}
                {fact.object}
              </p>
              <div className="mt-1 flex items-center gap-2">
                <ConfidenceBar confidence={fact.confidence} />
                <span className="text-ui-xs text-fg-secondary">
                  {Math.round(fact.confidence * 100)}%
                </span>
              </div>
            </div>
          ))}
        </Section>
      )}

      {/* Episodic Memories */}
      {context.episodicMemories.length > 0 && (
        <Section title="Episodic Memories" color="orange">
          {context.episodicMemories.map((mem) => (
            <div key={mem.id} className="rounded-md border border-brand/20 bg-brand/5 p-2">
              <p className="text-ui-sm text-brand">{mem.content}</p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-ui-xs text-fg-secondary">{mem.domain}</span>
                <span className="text-ui-xs text-fg-secondary">
                  {new Date(mem.createdAt).toLocaleDateString()}
                </span>
              </div>
            </div>
          ))}
        </Section>
      )}

      {/* Related Annotations */}
      {context.relatedAnnotations.length > 0 && (
        <Section title="Related Annotations" color="green">
          {context.relatedAnnotations.map((ann) => (
            <div key={ann.id} className="rounded-md border border-green-500/20 bg-green-500/5 p-2">
              <p className="text-ui-sm text-brand">{ann.content}</p>
              {ann.quotedText && (
                <p className="mt-1 text-ui-xs text-fg-secondary italic">"{ann.quotedText}"</p>
              )}
            </div>
          ))}
        </Section>
      )}

      {/* Procedural Rules */}
      {context.proceduralRules.length > 0 && (
        <Section title="Procedural Rules" color="blue">
          {context.proceduralRules.map((rule) => (
            <div key={rule.id} className="rounded-md border border-blue-500/20 bg-blue-500/5 p-2">
              <p className="text-ui-sm text-brand">{rule.ruleText}</p>
              <div className="mt-1 flex items-center justify-between">
                <span className="text-ui-xs text-fg-secondary">{rule.domain}</span>
                <span className="text-ui-xs text-fg-secondary">
                  {rule.signalCount} signal{rule.signalCount !== 1 ? "s" : ""}
                </span>
              </div>
            </div>
          ))}
        </Section>
      )}
    </div>
  );
}

function Section({
  title,
  color,
  children,
}: {
  title: string;
  color: string;
  children: React.ReactNode;
}) {
  const dotClass =
    color === "purple"
      ? "bg-purple-400"
      : color === "orange"
        ? "bg-brand"
        : color === "green"
          ? "bg-green-400"
          : "bg-blue-400";

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <div className={`h-2 w-2 rounded-full ${dotClass}`} />
        <span className="text-ui-xs font-medium text-fg-secondary">{title}</span>
      </div>
      {children}
    </div>
  );
}

function ConfidenceBar({ confidence }: { confidence: number }) {
  return (
    <div className="h-1 w-16 rounded-full bg-control-hover">
      <div
        className="h-full rounded-full bg-purple-400"
        style={{ width: `${Math.round(confidence * 100)}%` }}
      />
    </div>
  );
}
