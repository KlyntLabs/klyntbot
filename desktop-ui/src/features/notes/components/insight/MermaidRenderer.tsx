import mermaid from "mermaid";
import { useEffect, useId, useRef, useState } from "react";

/** Read a CSS custom property from :root as a resolved color string. */
function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Detect whether the current theme is light-on-white (e.g. retro/nexora). */
function isLightTheme(): boolean {
  const bg = getCssVar("--ds-bg");
  // Dark theme uses #000000 or similar; light themes use #ffffff / #fafafa
  return bg.startsWith("#f") || bg.startsWith("#e") || bg === "#ffffff";
}

function initMermaid() {
  const light = isLightTheme();

  mermaid.initialize({
    startOnLoad: false,
    theme: light ? "default" : "dark",
    themeVariables: light
      ? {
          primaryColor: getCssVar("--ds-accent") || "#ca8a04",
          primaryTextColor: getCssVar("--ds-text") || "#000000",
          primaryBorderColor: getCssVar("--ds-separator") || "#e5e5e5",
          lineColor: getCssVar("--ds-separator") || "#d4d4d4",
          secondaryColor: getCssVar("--ds-bg-elevated") || "#fafafa",
          tertiaryColor: getCssVar("--ds-bg") || "#f5f5f5",
        }
      : {
          primaryColor: "rgba(249, 115, 22, 0.3)",
          primaryTextColor: "#f0f2f5",
          primaryBorderColor: "rgba(255, 255, 255, 0.12)",
          lineColor: "rgba(255, 255, 255, 0.2)",
          secondaryColor: "rgba(255, 255, 255, 0.06)",
          tertiaryColor: "rgba(255, 255, 255, 0.04)",
        },
  });
}

// Initial call
initMermaid();

interface MermaidRendererProps {
  code: string;
  onError?: () => void;
}

export function MermaidRenderer({ code, onError }: MermaidRendererProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState(false);
  const uniqueId = useId().replace(/:/g, "-");

  useEffect(() => {
    if (!code.trim() || !containerRef.current) return;

    let cancelled = false;

    async function render() {
      try {
        // Re-initialize before each render to pick up theme changes
        initMermaid();
        const { svg } = await mermaid.render(`mermaid-${uniqueId}`, code);
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = svg;
          setError(false);
        }
      } catch {
        if (!cancelled) {
          setError(true);
          onError?.();
        }
      }
    }

    render();

    return () => {
      cancelled = true;
    };
  }, [code, uniqueId, onError]);

  if (error) return null;

  return <div ref={containerRef} className="w-full overflow-x-auto [&_svg]:max-w-full" />;
}
