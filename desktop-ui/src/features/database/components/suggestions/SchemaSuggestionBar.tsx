import { useSchemaSuggestions } from "@features/database/hooks/useSchemaSuggestions";
import { useMutation } from "@shared/hooks/useMutation";
import { SuggestionCard } from "./SuggestionCard";

interface SchemaSuggestionBarProps {
  databaseId: string;
}

export function SchemaSuggestionBar({ databaseId }: SchemaSuggestionBarProps) {
  const { data: suggestions } = useSchemaSuggestions(databaseId);
  const { mutate: resolveSuggestion } = useMutation<void, Record<string, unknown>>(
    "db_resolve_suggestion",
  );

  if (!suggestions || suggestions.length === 0) return null;

  const handleAccept = async (id: string) => {
    await resolveSuggestion({ database_id: databaseId, evolution_id: id, action: "accept" });
    window.dispatchEvent(new CustomEvent("database:updated"));
  };

  const handleDismiss = async (id: string) => {
    await resolveSuggestion({ database_id: databaseId, evolution_id: id, action: "dismiss" });
    window.dispatchEvent(new CustomEvent("database:updated"));
  };

  return (
    <div className="space-y-2 border-b border-border px-4 py-2">
      <div className="flex items-center gap-2">
        <span className="text-xs font-medium text-accent">✨ AI Suggestions</span>
        <span className="text-xs text-muted">{suggestions.length} pending</span>
      </div>
      {suggestions.slice(0, 3).map((s) => (
        <SuggestionCard
          key={s.id}
          suggestion={s}
          onAccept={handleAccept}
          onDismiss={handleDismiss}
        />
      ))}
    </div>
  );
}
