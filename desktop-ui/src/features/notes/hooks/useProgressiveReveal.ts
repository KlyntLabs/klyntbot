import type { Core, ElementDefinition } from "cytoscape";
import { useCallback, useRef } from "react";
import type { PositionMap } from "../lib/graphUtils";

interface RevealOptions {
  /** Milliseconds between waves (cache hit = fast, cache miss = slow) */
  waveDelay: number;
  /** Maximum number of animated waves before batching the rest */
  maxWaves: number;
  /** Whether to skip animation entirely */
  instant: boolean;
}

/**
 * Orchestrate wave-based progressive reveal on a Cytoscape instance.
 * Nodes are added in BFS waves with staggered opacity/scale animation.
 */
export function useProgressiveReveal() {
  const animationRef = useRef<number | null>(null);
  const isRevealingRef = useRef(false);

  const cancelReveal = useCallback(() => {
    if (animationRef.current !== null) {
      clearTimeout(animationRef.current);
      animationRef.current = null;
    }
    isRevealingRef.current = false;
  }, []);

  /**
   * Reveal elements wave by wave on a cache-hit (positions already known).
   * Purely visual — no layout computation, just staggered opacity + scale.
   */
  const revealWithPositions = useCallback(
    (
      cy: Core,
      waves: string[][],
      allElements: ElementDefinition[],
      positions: PositionMap,
      options: RevealOptions,
    ) => {
      cancelReveal();

      if (options.instant) {
        // Add all elements at once with cached positions
        const positioned = allElements.map((el) => {
          const pos = el.data?.id ? positions[el.data.id] : undefined;
          if (pos && el.group !== "edges") {
            return { ...el, position: pos };
          }
          return el;
        });
        cy.add(positioned);
        cy.nodes(":parent").ungrabify().unselectify();
        cy.fit(undefined, 40);
        return;
      }

      isRevealingRef.current = true;

      // Build element lookup by ID
      const elementById = new Map<string, ElementDefinition>();
      for (const el of allElements) {
        if (el.data?.id) elementById.set(el.data.id, el);
      }

      // Track which nodes have been revealed (for edge visibility)
      const revealedNodes = new Set<string>();
      let userInteracted = false;

      // Listen for user interaction to suppress auto-fit
      const onViewport = () => {
        userInteracted = true;
      };
      cy.on("viewport", onViewport);

      const revealWave = (waveIndex: number) => {
        if (waveIndex >= waves.length || !isRevealingRef.current) {
          isRevealingRef.current = false;
          cy.off("viewport", onViewport);
          return;
        }

        const wave = waves[waveIndex];

        // If beyond maxWaves, batch all remaining
        const nodeIds = waveIndex >= options.maxWaves ? waves.slice(waveIndex).flat() : wave;

        const batch: ElementDefinition[] = [];

        // Add compound parents first if any child references them
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el) continue;
          const parentId = el.data?.parent as string | undefined;
          if (parentId && !revealedNodes.has(parentId)) {
            const parentEl = elementById.get(parentId);
            if (parentEl) {
              batch.push(parentEl);
              revealedNodes.add(parentId);
            }
          }
        }

        // Add nodes with cached positions
        for (const id of nodeIds) {
          const el = elementById.get(id);
          if (!el || el.group === "edges") continue;
          const pos = positions[id];
          if (pos) {
            batch.push({ ...el, position: pos });
          } else {
            batch.push(el);
          }
          revealedNodes.add(id);
        }

        // Add edges where both endpoints are now revealed
        for (const el of allElements) {
          if (el.group !== "edges") continue;
          const src = el.data?.source as string;
          const tgt = el.data?.target as string;
          const edgeId = el.data?.id as string;
          if (
            revealedNodes.has(src) &&
            revealedNodes.has(tgt) &&
            !cy.getElementById(edgeId).nonempty()
          ) {
            batch.push(el);
          }
        }

        if (batch.length > 0) {
          const added = cy.add(batch);
          // Start at 70% of actual size + transparent, animate to full
          const childless = added.filter("node:childless");
          childless.forEach((node) => {
            const size = (node.data("size") as number) || 20;
            node.style({ opacity: 0, width: size * 0.7, height: size * 0.7 });
            void node.animate({
              style: { opacity: 1, width: size, height: size },
              duration: 200,
              easing: "ease-out",
            });
          });

          // Auto-fit (only if user hasn't panned/zoomed)
          if (!userInteracted) {
            cy.animate({ fit: { eles: cy.nodes(), padding: 40 }, duration: 200 });
          }

          // Hub pulse effect on wave 0
          if (waveIndex === 0) {
            const hubNode = childless.first();
            if (hubNode.nonempty()) {
              hubNode.addClass("hub-pulse");
              setTimeout(() => hubNode.removeClass("hub-pulse"), 800);
            }
          }
        }

        // Make compound parents non-interactive
        cy.nodes(":parent").ungrabify().unselectify();

        // Schedule next wave (or finish if we batched remaining)
        if (waveIndex >= options.maxWaves) {
          isRevealingRef.current = false;
          cy.off("viewport", onViewport);
          return;
        }

        animationRef.current = window.setTimeout(
          () => revealWave(waveIndex + 1),
          options.waveDelay,
        );
      };

      revealWave(0);
    },
    [cancelReveal],
  );

  return { revealWithPositions, cancelReveal, isRevealing: isRevealingRef };
}
