import { useInsightChat } from "@features/notes/hooks/useInsightChat";
import { useCopyToClipboard } from "@shared/hooks/useCopyToClipboard";
import { ClipboardCopy } from "lucide-react";
import { lazy, Suspense, useCallback, useState } from "react";
import type { TabStatus } from "../../hooks/useInsightReview";
import { InsightChatInput } from "./InsightChatInput";

const MermaidRenderer = lazy(() =>
  import("./MermaidRenderer").then((m) => ({ default: m.MermaidRenderer })),
);

interface ConceptMapTabProps {
  status: TabStatus;
  mermaid: string;
  fallbackText: string;
  noteId: string | null;
  squadId?: string | null;
}

function SkeletonLoader() {
  return (
    <div className="space-y-4 animate-pulse">
      <div className="h-40 bg-card rounded-lg" />
      <div className="h-3 bg-card rounded w-1/2 mx-auto" />
    </div>
  );
}

function TextOutline({ text }: { text: string }) {
  // Render as a simple indented text outline
  const lines = text.split("\n").filter(Boolean);
  return (
    <div className="space-y-1 font-mono text-[11px] text-muted-foreground">
      {lines.map((line, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: static derived list with no reordering
        <div key={i} className="whitespace-pre-wrap">
          {line}
        </div>
      ))}
    </div>
  );
}

function CopyButton({
  mermaidCode,
  copied,
  onCopy,
}: {
  mermaidCode: string;
  copied: boolean;
  onCopy: () => void;
}) {
  if (!mermaidCode) return null;
  return (
    <button
      type="button"
      onClick={onCopy}
      className="flex items-center gap-1.5 text-2xs text-muted-foreground hover:text-foreground transition-colors"
    >
      <ClipboardCopy size={10} />
      {copied ? "Copied!" : "Copy Mermaid code"}
    </button>
  );
}

export function ConceptMapTab({
  status,
  mermaid: mermaidCode,
  fallbackText,
  noteId,
  squadId,
}: ConceptMapTabProps) {
  const [renderFailed, setRenderFailed] = useState(false);
  const { copied, copy } = useCopyToClipboard();
  const chat = useInsightChat(
    noteId,
    "concept-map",
    status === "done",
    squadId,
    mermaidCode || fallbackText,
  );

  const handleCopy = useCallback(async () => {
    await copy(mermaidCode);
  }, [mermaidCode, copy]);

  const handleRenderError = useCallback(() => {
    setRenderFailed(true);
  }, []);

  if (status === "idle") {
    return (
      <p className="text-[11px] text-dim italic">Start an insight review to see the concept map</p>
    );
  }

  if (status === "loading" || status === "streaming") {
    return <SkeletonLoader />;
  }

  if (status === "error") {
    return (
      <p className="text-[11px] text-destructive">
        Failed to generate concept map. Try regenerating.
      </p>
    );
  }

  // Fallback text mode
  if (fallbackText || renderFailed) {
    const displayText = fallbackText || mermaidCode;
    return (
      <div>
        <div className="space-y-3">
          <TextOutline text={displayText} />
          {mermaidCode && !renderFailed && (
            <CopyButton mermaidCode={mermaidCode} copied={copied} onCopy={handleCopy} />
          )}
        </div>
        {status === "done" && (
          <InsightChatInput {...chat} placeholder="Ask about this concept map..." />
        )}
      </div>
    );
  }

  // Mermaid diagram mode
  if (mermaidCode) {
    return (
      <div>
        <div className="space-y-3">
          <div className="rounded-lg bg-card border border-border p-4">
            <Suspense
              fallback={
                <div className="flex items-center justify-center h-full text-muted text-sm">
                  Loading...
                </div>
              }
            >
              <MermaidRenderer code={mermaidCode} onError={handleRenderError} />
            </Suspense>
          </div>
          <CopyButton mermaidCode={mermaidCode} copied={copied} onCopy={handleCopy} />
        </div>
        {status === "done" && (
          <InsightChatInput {...chat} placeholder="Ask about this concept map..." />
        )}
      </div>
    );
  }

  return <p className="text-[11px] text-dim italic">No concept map data</p>;
}
