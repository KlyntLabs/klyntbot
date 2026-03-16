import mermaid from "mermaid";
import { useEffect, useId, useRef, useState } from "react";

// Initialize mermaid with dark theme matching glassmorphism design
mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  themeVariables: {
    primaryColor: "rgba(249, 115, 22, 0.3)",
    primaryTextColor: "#f0f2f5",
    primaryBorderColor: "rgba(255, 255, 255, 0.12)",
    lineColor: "rgba(255, 255, 255, 0.2)",
    secondaryColor: "rgba(255, 255, 255, 0.06)",
    tertiaryColor: "rgba(255, 255, 255, 0.04)",
  },
});

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
