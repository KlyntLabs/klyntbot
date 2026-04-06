import { useCallback, useRef } from "react";
import type { ForceGraphMethods } from "react-force-graph-3d";
import { Mesh } from "three";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";
import { getPooledGeometry, getPooledMaterial } from "../lib/geometryPool";
import type { ForceNode } from "./useGraphElements";
import type { GraphSettings } from "./useGraphSettings";

interface UseBrainViewParams {
  settings: GraphSettings;
}

export function useBrainView({ settings }: UseBrainViewParams) {
  const graphRef = useRef<ForceGraphMethods>(undefined);
  const bloomAddedRef = useRef(false);
  const rotationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isRotatingRef = useRef(false);

  // Build a custom Three.js object for each node, branching on nodeType
  const nodeThreeObject = useCallback((node: ForceNode) => {
    const nodeType = node.nodeType ?? "note";
    let emissiveIntensity: number;

    switch (nodeType) {
      case "entity":
        emissiveIntensity = 0.6;
        break;
      case "tree_section":
        emissiveIntensity = 0.2;
        break;
      case "tree_text":
        emissiveIntensity = 0.1;
        break;
      case "finance":
        emissiveIntensity = 0.55;
        break;
      case "productivity":
        emissiveIntensity = 0.5;
        break;
      case "okr":
        emissiveIntensity = 0.6;
        break;
      case "learning":
        emissiveIntensity = 0.5;
        break;
      case "project":
        emissiveIntensity = 0.55;
        break;
      default:
        emissiveIntensity = Math.min(0.3 + (0.7 * node.linkCount) / 15, 1);
        break;
    }

    const geometry = getPooledGeometry(nodeType, node.size, node.linkCount);
    const material = getPooledMaterial(nodeType, node.color, emissiveIntensity);
    const mesh = new Mesh(geometry, material);
    mesh.userData = { nodeId: node.id };
    return mesh;
  }, []);

  // Add bloom post-processing to the ForceGraph3D instance
  const setupPostProcessing = useCallback(() => {
    const fg = graphRef.current;
    if (!fg || bloomAddedRef.current) return;

    try {
      const composer = fg.postProcessingComposer();
      if (!composer) return;

      const bloomPass = new UnrealBloomPass(
        { x: 1024, y: 1024 } as never, // resolution (Vector2-like)
        1.2, // strength
        0.4, // radius
        0.85, // threshold
      );
      composer.addPass(bloomPass);
      bloomAddedRef.current = true;
    } catch {
      // Post-processing not available in this environment
    }
  }, []);

  // Handle idle auto-rotation
  const startIdleRotation = useCallback(() => {
    const fg = graphRef.current;
    if (!fg || !settings.idleRotation) return;

    const controls = fg.controls() as { autoRotate?: boolean; autoRotateSpeed?: number };
    if (controls && "autoRotate" in controls) {
      controls.autoRotate = true;
      controls.autoRotateSpeed = 0.5;
      isRotatingRef.current = true;
    }
  }, [settings.idleRotation]);

  const stopIdleRotation = useCallback(() => {
    const fg = graphRef.current;
    if (!fg || !isRotatingRef.current) return;

    const controls = fg.controls() as { autoRotate?: boolean };
    if (controls && "autoRotate" in controls) {
      controls.autoRotate = false;
      isRotatingRef.current = false;
    }
  }, []);

  const resetIdleTimer = useCallback(() => {
    if (rotationTimerRef.current) {
      clearTimeout(rotationTimerRef.current);
    }
    stopIdleRotation();

    if (settings.idleRotation) {
      rotationTimerRef.current = setTimeout(startIdleRotation, 5000);
    }
  }, [settings.idleRotation, startIdleRotation, stopIdleRotation]);

  return {
    graphRef,
    nodeThreeObject,
    setupPostProcessing,
    resetIdleTimer,
  };
}
