import { useCallback, useRef } from "react";
import type { ForceGraphMethods } from "react-force-graph-3d";
import { Mesh } from "three";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";
import { createNodeGeometry, createNodeMaterial } from "../lib/graphMaterials";
import type { ForceNode } from "./useGraphElements";
import type { GraphSettings } from "./useGraphSettings";

interface UseBrainViewParams {
  settings: GraphSettings;
}

export function useBrainView({ settings }: UseBrainViewParams) {
  const graphRef = useRef<ForceGraphMethods>();
  const bloomAddedRef = useRef(false);
  const rotationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isRotatingRef = useRef(false);

  // Build a custom Three.js Mesh for each node
  const nodeThreeObject = useCallback(
    (node: ForceNode) => {
      const emissiveIntensity = Math.min(0.3 + (0.7 * node.linkCount) / 15, 1);
      const geometry = createNodeGeometry(node.size * 0.3); // scale down for 3D space
      const material = createNodeMaterial(node.color, emissiveIntensity);
      return new Mesh(geometry, material);
    },
    [],
  );

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
