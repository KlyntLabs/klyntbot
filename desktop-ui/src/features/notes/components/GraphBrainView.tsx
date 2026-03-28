import { useEffect } from "react";
import ForceGraph3D from "react-force-graph-3d";
import { useBrainView } from "../hooks/useBrainView";
import type { GraphElements } from "../hooks/useGraphElements";
import type { GraphSettings } from "../hooks/useGraphSettings";

interface GraphBrainViewProps {
  elements: GraphElements;
  settings: GraphSettings;
  width: number;
  height: number;
  onNodeClick?: (id: string) => void;
  onNodeHover?: (id: string | null, x: number, y: number) => void;
}

export function GraphBrainView({
  elements,
  settings,
  width,
  height,
  onNodeClick,
  onNodeHover,
}: GraphBrainViewProps) {
  const { graphRef, nodeThreeObject, setupPostProcessing, resetIdleTimer } = useBrainView({
    settings,
  });

  // Set up bloom post-processing once the graph mounts
  useEffect(() => {
    // Small delay to ensure the renderer is ready
    const timer = setTimeout(setupPostProcessing, 100);
    return () => clearTimeout(timer);
  }, [setupPostProcessing]);

  // Start idle rotation timer on mount
  useEffect(() => {
    resetIdleTimer();
  }, [resetIdleTimer]);

  const graphData = {
    nodes: elements.nodes,
    links: elements.links,
  };

  return (
    <ForceGraph3D
      ref={graphRef as never}
      graphData={graphData as never}
      width={width}
      height={height}
      backgroundColor="#07070d"
      nodeThreeObject={nodeThreeObject as never}
      nodeThreeObjectExtend={false}
      linkColor={((link: { color?: string }) => link.color || "#4B5563") as never}
      linkOpacity={0.35}
      linkWidth={0.5}
      linkDirectionalParticles={settings.showArrows ? 2 : 0}
      linkDirectionalParticleSpeed={0.005}
      linkDirectionalParticleWidth={0.8}
      showNavInfo={false}
      onNodeClick={
        ((node: { id?: string }) => {
          if (node.id && onNodeClick) {
            onNodeClick(String(node.id));
          }
          resetIdleTimer();
        }) as never
      }
      onNodeHover={
        ((node: { id?: string } | null) => {
          if (onNodeHover) {
            if (node?.id) {
              // In 3D mode we pass 0,0 for x,y since tooltip positioning differs
              onNodeHover(String(node.id), 0, 0);
            } else {
              onNodeHover(null, 0, 0);
            }
          }
        }) as never
      }
      onNodeDrag={(() => resetIdleTimer()) as never}
      cooldownTicks={settings.livePhysics ? Infinity : 100}
      enableNodeDrag={true}
      enableNavigationControls={true}
    />
  );
}
