import { useCopyToClipboard } from "@shared/hooks/useCopyToClipboard";
import { ClipboardCopy } from "lucide-react";
import { useCallback, useState } from "react";
import type { TabStatus } from "../../hooks/useInsightReview";
import { MermaidRenderer } from "./MermaidRenderer";

interface ConceptMapTabProps {
  status: TabStatus;
  mermaid: string;
  fallbackText: string;
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

export function ConceptMapTab({ status, mermaid: mermaidCode, fallbackText }: ConceptMapTabProps) {
  const [renderFailed, setRenderFailed] = useState(false);
  const { copied, copy } = useCopyToClipboard();

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
      <div className="space-y-3">
        <TextOutline text={displayText} />
        {mermaidCode && !renderFailed && (
          <CopyButton mermaidCode={mermaidCode} copied={copied} onCopy={handleCopy} />
        )}
      </div>
    );
  }

  // Mermaid diagram mode
  if (mermaidCode) {
    return (
      <div className="space-y-3">
        <div className="rounded-lg bg-card border border-border p-4">
          <MermaidRenderer code={mermaidCode} onError={handleRenderError} />
        </div>
        <CopyButton mermaidCode={mermaidCode} copied={copied} onCopy={handleCopy} />
      </div>
    );
  }

  return <p className="text-[11px] text-dim italic">No concept map data</p>;
}
