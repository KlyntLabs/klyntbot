import type { PersonaMeta, TabStatus } from "../../hooks/useInsightReview";
import { PersonaCard } from "./PersonaCard";

interface PerspectivesTabProps {
  status: TabStatus;
  content: string;
  personas: PersonaMeta[];
  noteId?: string | null;
}

/** Parse the perspectives markdown into per-persona sections by splitting on `---` and `## ` headings. */
function parsePersonaSections(
  content: string,
  personas: PersonaMeta[],
): { persona: PersonaMeta; section: string }[] {
  if (!content || personas.length === 0) return [];

  // Split by horizontal rules (---) which separate persona sections
  const sections = content.split(/\n---\n/).filter((s) => s.trim().length > 0);

  return personas.map((persona, i) => ({
    persona,
    section: sections[i]?.trim() ?? "",
  }));
}

function SkeletonLoader() {
  return (
    <div className="space-y-4">
      {[1, 2, 3].map((i) => (
        <div key={i} className="glass-card rounded-lg p-3 space-y-2 animate-pulse">
          <div className="flex items-center gap-2">
            <div className="w-7 h-7 rounded-full bg-card" />
            <div className="space-y-1">
              <div className="h-3 bg-card rounded w-24" />
              <div className="h-2 bg-card rounded w-16" />
            </div>
          </div>
          <div className="h-3 bg-card rounded w-full" />
          <div className="h-3 bg-card rounded w-4/5" />
          <div className="h-3 bg-card rounded w-3/4" />
        </div>
      ))}
    </div>
  );
}

export function PerspectivesTab({ status, content, personas, noteId }: PerspectivesTabProps) {
  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">
        Start an insight review to see expert perspectives
      </p>
    );
  }

  if (status === "loading") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate perspectives. Try regenerating.
      </p>
    );
  }

  const sections = parsePersonaSections(content, personas);

  // Fallback: if parsing failed or no personas, render full markdown
  if (sections.length === 0 && content) {
    return (
      <div className="space-y-4">
        <div className="text-[10px] text-dim italic">
          Perspectives (persona details unavailable)
        </div>
        <div className="text-[12px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
          {content}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {sections.map(
        ({ persona, section }) =>
          section && (
            <PersonaCard
              key={persona.id}
              name={persona.name}
              role={persona.role}
              icon={persona.icon}
              tone={persona.tone}
              content={section}
              noteId={noteId ?? undefined}
              personaId={persona.id}
            />
          ),
      )}
    </div>
  );
}
