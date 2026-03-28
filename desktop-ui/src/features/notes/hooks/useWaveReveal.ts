import { useCallback, useRef, useState } from "react";
import { computeBfsWaves } from "../lib/graphBfs";
import type { ForceNode, GraphElements } from "./useGraphElements";
import type { PositionMap } from "./useGraphPositionCache";

const WAVE_DELAYS = {
  instant: 0,
  balanced: 80,
  cinematic: 150,
} as const;

const MAX_ANIMATED_WAVES = 5;

export interface WaveRevealController {
  revealWave: (
    hubId: string,
    elements: GraphElements,
    cachedPositions?: PositionMap | null,
    waveOrder?: string[][],
  ) => void;
  triggerMicroReveal: (nodeIds: string[]) => void;
  revealProgress: number;
  isRevealing: boolean;
  cancelReveal: () => void;
  revealedNodes: Set<string>;
}

type RevealSpeed = "instant" | "balanced" | "cinematic";

export function useWaveReveal(revealSpeed: RevealSpeed): WaveRevealController {
  const [revealProgress, setRevealProgress] = useState(1);
  const [isRevealing, setIsRevealing] = useState(false);
  const revealedNodesRef = useRef<Set<string>>(new Set());
  const timersRef = useRef<ReturnType<typeof setTimeout>[]>([]);
  const microRevealTimersRef = useRef<ReturnType<typeof setTimeout>[]>([]);

  const cancelReveal = useCallback(() => {
    for (const t of timersRef.current) clearTimeout(t);
    timersRef.current = [];
    setIsRevealing(false);
    setRevealProgress(1);
  }, []);

  const revealWave = useCallback(
    (
      hubId: string,
      elements: GraphElements,
      _cachedPositions?: PositionMap | null,
      waveOrder?: string[][],
    ) => {
      cancelReveal();

      const allNodeIds = new Set(elements.nodes.map((n) => n.id));
      if (allNodeIds.size === 0) return;

      const waves =
        waveOrder ??
        (() => {
          const adjacency = new Map<string, Set<string>>();
          for (const link of elements.links) {
            const sId =
              typeof link.source === "string"
                ? link.source
                : (link.source as unknown as ForceNode).id;
            const tId =
              typeof link.target === "string"
                ? link.target
                : (link.target as unknown as ForceNode).id;
            if (!adjacency.has(sId)) adjacency.set(sId, new Set());
            if (!adjacency.has(tId)) adjacency.set(tId, new Set());
            adjacency.get(sId)?.add(tId);
            adjacency.get(tId)?.add(sId);
          }
          return computeBfsWaves(hubId, adjacency, allNodeIds);
        })();

      if (waves.length === 0) return;

      const totalNodes = allNodeIds.size;
      const delay = WAVE_DELAYS[revealSpeed];

      if (delay === 0) {
        revealedNodesRef.current = new Set(allNodeIds);
        setRevealProgress(1);
        setIsRevealing(false);
        return;
      }

      setIsRevealing(true);
      setRevealProgress(0);
      revealedNodesRef.current = new Set();
      let revealedCount = 0;

      const revealNextWave = (waveIndex: number) => {
        if (waveIndex >= waves.length) {
          setIsRevealing(false);
          setRevealProgress(1);
          return;
        }

        const nodeIds =
          waveIndex >= MAX_ANIMATED_WAVES ? waves.slice(waveIndex).flat() : waves[waveIndex];

        for (const id of nodeIds) {
          revealedNodesRef.current.add(id);
        }
        revealedCount += nodeIds.length;
        setRevealProgress(Math.min(revealedCount / totalNodes, 1));

        const isFinal = waveIndex >= MAX_ANIMATED_WAVES || waveIndex >= waves.length - 1;
        if (isFinal) {
          for (const id of allNodeIds) revealedNodesRef.current.add(id);
          setRevealProgress(1);
          setIsRevealing(false);
        } else {
          const timer = setTimeout(() => revealNextWave(waveIndex + 1), delay);
          timersRef.current.push(timer);
        }
      };

      revealNextWave(0);
    },
    [revealSpeed, cancelReveal],
  );

  const triggerMicroReveal = useCallback((nodeIds: string[]) => {
    for (const t of microRevealTimersRef.current) clearTimeout(t);
    microRevealTimersRef.current = [];

    for (const id of nodeIds) {
      revealedNodesRef.current.add(id);
    }
  }, []);

  return {
    revealWave,
    triggerMicroReveal,
    revealProgress,
    isRevealing,
    cancelReveal,
    revealedNodes: revealedNodesRef.current,
  };
}
