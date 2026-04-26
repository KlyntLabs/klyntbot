import { useState } from "react";
import ReactDiffViewer from "react-diff-viewer-continued";
import { useReforgeCycleDiff, useReforgeCycleList } from "./hooks";

export function ReforgeCycleDiffPanel() {
  const [repoId, setRepoId] = useState("");
  const [artifact, setArtifact] = useState("claude_md");
  const [beforeCycleId, setBeforeCycleId] = useState("");
  const [afterCycleId, setAfterCycleId] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const cycles = useReforgeCycleList();
  const diff = useReforgeCycleDiff(
    repoId,
    artifact,
    beforeCycleId || undefined,
    afterCycleId || undefined,
  );

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitted(true);
  };

  return (
    <section className="p-6 space-y-4 h-full overflow-auto">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-medium text-foreground">Reforge Cycle Diff</h2>
      </header>
      <form onSubmit={handleSubmit} className="flex flex-wrap gap-2 items-end">
        <label className="flex flex-col gap-1 text-[11px] uppercase tracking-wider text-muted-foreground">
          Repo
          <input
            type="text"
            placeholder="Repo ID"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm text-foreground normal-case tracking-normal"
            value={repoId}
            onChange={(e) => setRepoId(e.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-[11px] uppercase tracking-wider text-muted-foreground">
          Artifact
          <select
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm text-foreground normal-case tracking-normal"
            value={artifact}
            onChange={(e) => setArtifact(e.target.value)}
          >
            <option value="claude_md">claude_md</option>
            <option value="agents_md">agents_md</option>
            <option value="cursorrules">cursorrules</option>
            <option value="continue_rules">continue_rules</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-[11px] uppercase tracking-wider text-muted-foreground">
          Before cycle
          <input
            type="text"
            placeholder="Cycle ID"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm text-foreground normal-case tracking-normal"
            value={beforeCycleId}
            onChange={(e) => setBeforeCycleId(e.target.value)}
          />
        </label>
        <label className="flex flex-col gap-1 text-[11px] uppercase tracking-wider text-muted-foreground">
          After cycle
          <input
            type="text"
            placeholder="Cycle ID (optional)"
            className="bg-surface-base border border-border rounded px-2 py-1 text-sm text-foreground normal-case tracking-normal"
            value={afterCycleId}
            onChange={(e) => setAfterCycleId(e.target.value)}
          />
        </label>
        <button
          type="submit"
          className="rounded-lg border border-border px-3 py-1 text-sm text-foreground hover:bg-accent transition-colors"
        >
          Diff
        </button>
      </form>

      {cycles.data && cycles.data.length > 0 && (
        <div className="text-sm text-muted-foreground">
          Recent cycles:{" "}
          {cycles.data.slice(0, 5).map((c) => (
            <button
              key={c.cycleId}
              type="button"
              className="ml-2 text-brand hover:underline"
              onClick={() => setBeforeCycleId(c.cycleId)}
            >
              {c.cycleId}
            </button>
          ))}
        </div>
      )}

      {submitted && diff.loading && <div className="text-sm text-muted-foreground">Loading…</div>}
      {submitted && diff.error && (
        <div className="text-sm text-destructive">Error: {String(diff.error)}</div>
      )}
      {submitted && diff.data && (
        <div className="border border-border rounded-xl overflow-hidden">
          <ReactDiffViewer
            oldValue={diff.data.beforeBody}
            newValue={diff.data.afterBody}
            splitView={false}
            useDarkTheme
            leftTitle="Before"
            rightTitle="After"
          />
        </div>
      )}
    </section>
  );
}
