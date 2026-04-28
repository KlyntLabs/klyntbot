import { useEffect, useId, useState } from "react";

type MermaidState =
  | { status: "loading" }
  | { status: "ready"; svg: string }
  | { status: "error"; message: string };

type MermaidApi = typeof import("mermaid").default;

let mermaidPromise: Promise<MermaidApi> | null = null;

function resolveTheme() {
  const explicit = document.documentElement.getAttribute("data-theme");
  if (explicit === "light") {
    return "default" as const;
  }
  if (explicit === "dim" || explicit === "dark") {
    return "dark" as const;
  }
  if (typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: light)").matches) {
    return "default" as const;
  }
  return "dark" as const;
}

function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then((mod) => {
      const mermaid = mod.default;
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: resolveTheme(),
        fontFamily:
          "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
      });
      return mermaid;
    });
  }
  return mermaidPromise;
}

export function MermaidDiagram({ source }: { source: string }) {
  const [state, setState] = useState<MermaidState>({ status: "loading" });
  const reactId = useId();
  const safeId = `mermaid-${reactId.replace(/[^a-zA-Z0-9_-]/g, "_")}`;

  useEffect(() => {
    let cancelled = false;
    setState({ status: "loading" });
    loadMermaid()
      .then(async (mermaid) => {
        try {
          const parsed = await mermaid.parse(source, { suppressErrors: true });
          if (parsed === false) {
            if (!cancelled) {
              setState({ status: "error", message: "Invalid Mermaid syntax" });
            }
            return;
          }
          const { svg } = await mermaid.render(safeId, source);
          if (!cancelled) {
            setState({ status: "ready", svg });
          }
        } catch (err) {
          if (!cancelled) {
            setState({
              status: "error",
              message: err instanceof Error ? err.message : String(err),
            });
          }
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setState({
            status: "error",
            message: err instanceof Error ? err.message : String(err),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [source, safeId]);

  if (state.status === "loading") {
    return (
      <div
        className="markdown-mermaid markdown-mermaid-loading"
        role="status"
        aria-live="polite"
      >
        Rendering diagram…
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <div className="markdown-mermaid markdown-mermaid-error" role="alert">
        <div className="markdown-mermaid-error-label">
          Mermaid render error: {state.message}
        </div>
        <pre tabIndex={0}>
          <code>{source}</code>
        </pre>
      </div>
    );
  }

  return (
    <div
      className="markdown-mermaid"
      role="img"
      aria-label="Mermaid diagram"
      tabIndex={0}
      // biome-ignore lint/security/noDangerouslySetInnerHtml: mermaid sanitizes via securityLevel: strict
      dangerouslySetInnerHTML={{ __html: state.svg }}
    />
  );
}
