import { type CodingMode, useCodingMode } from "../hooks/useCodingMode";

export function CodingModePill({ threadId }: { threadId: string | null }) {
  const { mode, setMode, loading } = useCodingMode(threadId);

  const next: CodingMode = mode === "coding" ? "general" : "coding";
  return (
    <button
      type="button"
      className={`coding-mode-pill ${mode === "coding" ? "is-coding" : "is-general"}`}
      disabled={loading || !threadId}
      onClick={() => setMode(next)}
      aria-label={`Switch to ${next} mode`}
    >
      {mode === "coding" ? "Coding" : "General"}
    </button>
  );
}
