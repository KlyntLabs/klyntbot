interface TranslationSectionProps {
  translation: string | null;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}

export function TranslationSection({
  translation,
  loading,
  error,
  onRetry,
}: TranslationSectionProps) {
  return (
    <div className="border-b border-border px-3 py-3">
      <div className="text-[10px] text-muted-foreground uppercase tracking-wider mb-2">
        Translation
      </div>
      {loading && (
        <div className="space-y-2">
          <div className="h-4 bg-surface-hover rounded animate-pulse" />
          <div className="h-4 bg-surface-hover rounded animate-pulse w-3/4" />
        </div>
      )}
      {error && (
        <div className="text-xs text-red-400">
          {error}{" "}
          <button type="button" onClick={onRetry} className="text-brand underline">
            Retry
          </button>
        </div>
      )}
      {translation && !loading && (
        <div className="rounded-md border-l-2 border-brand bg-surface-hover/50 px-3 py-2">
          <p className="text-sm text-primary leading-relaxed">{translation}</p>
        </div>
      )}
    </div>
  );
}
