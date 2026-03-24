import { ThinkingDots } from "@shared/ui/ThinkingDots";

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
      <div className="flex items-center gap-2 mb-2">
        <div className="text-2xs text-muted-foreground uppercase tracking-wider">Translation</div>
        {loading && <ThinkingDots size="sm" />}
      </div>
      {loading && !translation && <TranslationSkeleton />}
      {error && (
        <div className="text-xs text-red-400">
          {error}{" "}
          <button type="button" onClick={onRetry} className="text-brand underline">
            Retry
          </button>
        </div>
      )}
      {translation && (
        <div
          className={`rounded-md border-l-2 border-brand bg-surface-hover/50 px-3 py-2 transition-opacity ${loading ? "opacity-40" : ""}`}
        >
          <p className="text-sm text-primary leading-relaxed">{translation}</p>
        </div>
      )}
    </div>
  );
}

function TranslationSkeleton() {
  return (
    <div className="space-y-3">
      {/* Translation block skeleton */}
      <div className="rounded-md border-l-2 border-border bg-surface-hover/30 px-3 py-2 space-y-2">
        <div className="h-3.5 rounded bg-surface-hover animate-[shimmer_2s_infinite]" />
        <div className="h-3.5 rounded bg-surface-hover animate-[shimmer_2s_0.15s_infinite] w-[90%]" />
        <div className="h-3.5 rounded bg-surface-hover animate-[shimmer_2s_0.3s_infinite] w-[75%]" />
      </div>
      {/* Words skeleton */}
      <div className="space-y-1.5 pt-1">
        {Array.from({ length: 5 }).map((_, i) => (
          <div key={i} className="flex items-center gap-2">
            <div
              className="h-3 rounded bg-surface-hover animate-[shimmer_2s_infinite] w-12"
              style={{ animationDelay: `${i * 0.1}s` }}
            />
            <div
              className="h-2.5 rounded bg-surface-hover/60 animate-[shimmer_2s_infinite] flex-1"
              style={{ animationDelay: `${i * 0.1 + 0.05}s` }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
