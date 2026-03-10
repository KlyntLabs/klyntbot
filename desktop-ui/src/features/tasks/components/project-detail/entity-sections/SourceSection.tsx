import { CollapsibleSection } from "@shared/components";
import { useProjectSources } from "@shared/hooks";
import { ExternalLink, Link2, Package } from "lucide-react";

interface SourceSectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

export function SourceSection({ projectId, defaultOpen }: SourceSectionProps) {
  const { data: sources } = useProjectSources(projectId);

  return (
    <CollapsibleSection
      title="Sources"
      icon={<Package className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      count={sources.length || null}
      defaultOpen={defaultOpen}
    >
      {sources.length === 0 ? (
        <p className="text-[11px] text-dim font-light py-2">No sources</p>
      ) : (
        <div className="space-y-0.5">
          {sources.map((source) => (
            <div
              key={source.id}
              className="flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-white/[0.04] transition-colors"
            >
              {source.url ? (
                <ExternalLink className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
              ) : (
                <Link2 className="w-3 h-3 text-muted shrink-0" strokeWidth={1.5} />
              )}
              <span className="text-[11px] font-light text-secondary truncate">{source.title}</span>
              <span className="text-[9px] text-dim ml-auto shrink-0">{source.sourceType}</span>
            </div>
          ))}
        </div>
      )}
    </CollapsibleSection>
  );
}
