import { useEffect, useRef } from "react";
import { useKnowledgeHealth } from "../hooks/useKnowledgeHealth";

export function AtomGraph() {
  const containerRef = useRef<HTMLDivElement>(null);
  const { data: health } = useKnowledgeHealth();

  useEffect(() => {
    if (!containerRef.current || health.topics.length === 0) return;

    let cy: import("cytoscape").Core | null = null;

    // Dynamic import to avoid SSR/bundle issues
    import("cytoscape").then(({ default: cytoscape }) => {
      if (!containerRef.current) return;

      const nodes = health.topics.map((t) => ({
        data: {
          id: t.id,
          label: t.name,
          size: Math.max(20, t.atomCount * 4),
          retention: t.avgRetention,
        },
      }));

      cy = cytoscape({
        container: containerRef.current,
        elements: [...nodes],
        style: [
          {
            selector: "node",
            style: {
              label: "data(label)",
              width: "data(size)",
              height: "data(size)",
              "background-color": "var(--brand)",
              "background-opacity": 0.7,
              "border-width": 2,
              "border-color": "var(--brand)",
              color: "var(--muted)",
              "font-size": "10px",
              "text-valign": "bottom",
              "text-margin-y": 4,
              "text-outline-width": 0,
            },
          },
        ],
        layout: { name: "cose", animate: false },
        userZoomingEnabled: true,
        userPanningEnabled: true,
      });
    });

    return () => {
      cy?.destroy();
    };
  }, [health.topics]);

  if (health.topics.length === 0) {
    return (
      <div className="flex items-center justify-center h-[400px] text-[11px] text-muted-foreground">
        No topics yet. Accept knowledge atoms from your notes to see the graph.
      </div>
    );
  }

  return <div ref={containerRef} className="w-full h-[400px]" />;
}
