import type { Stylesheet } from "cytoscape";
import { useEffect, useMemo, useState } from "react";

function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Resolve a CSS variable to a computed hex/rgb color that Cytoscape can understand.
 *  CSS vars may use oklch() or other color spaces that Cytoscape doesn't support.
 *  We create a temporary element, apply the color, and read the computed value. */
function _resolveColor(varName: string, fallback: string): string {
  const raw = getCssVar(varName);
  if (!raw) return fallback;
  // If already hex or rgb, return directly
  if (raw.startsWith("#") || raw.startsWith("rgb")) return raw;
  // Resolve via temporary element
  const el = document.createElement("div");
  el.style.color = raw;
  document.body.appendChild(el);
  const resolved = getComputedStyle(el).color;
  document.body.removeChild(el);
  return resolved || fallback;
}

function _isLightTheme(): boolean {
  const bg = getCssVar("--background");
  return bg.startsWith("#f") || bg.startsWith("#e") || bg === "#ffffff";
}

export function useCytoscapeTheme(): { stylesheet: Stylesheet[]; isLight: boolean } {
  const [themeKey, setThemeKey] = useState(
    () => document.documentElement.getAttribute("data-theme") || "dark",
  );

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeKey(document.documentElement.getAttribute("data-theme") || "dark");
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return useMemo(() => {
    const light = themeKey === "retro";
    const textPrimary = light ? "#000000" : "#f0f2f5";
    const textSecondary = light ? "#525252" : "#c8cdd4";
    const _border = light ? "#d4d4d4" : "rgba(255,255,255,0.1)";
    const edgeColor = light ? "#d4d4d4" : "rgba(255,255,255,0.15)";
    const brand = light ? "#ca8a04" : "#f97316";

    const stylesheet: Stylesheet[] = [
      // ── Compound parents — invisible but used by fCoSE for spatial grouping ──
      {
        selector: "node:parent",
        style: {
          "background-opacity": 0,
          "border-width": 0,
          "border-opacity": 0,
          label: "",
          padding: "20px",
          events: "no",
          "overlay-opacity": 0,
        },
      },

      // ── Nodes — colored circles with right-aligned labels ──
      {
        selector: "node:childless",
        style: {
          label: "data(label)",
          width: "data(size)",
          height: "data(size)",
          "background-color": "data(color)",
          "background-opacity": 0.85,
          "border-width": 2,
          "border-color": "data(color)",
          "border-opacity": 0.2,
          // Label styling — positioned to the right like MiroFish
          "font-size": 11,
          "font-weight": "500",
          "font-family": "Inter, system-ui, sans-serif",
          color: textSecondary,
          "text-valign": "center",
          "text-halign": "right",
          "text-margin-x": 6,
          "text-max-width": "120px",
          "text-wrap": "ellipsis",
          "min-zoomed-font-size": 8,
          "text-opacity": 1,
          // Smooth transitions
          "transition-property":
            "background-color, border-color, width, height, opacity, border-width",
          "transition-duration": 180,
        },
      },

      // ── Hide labels at low zoom ──
      {
        selector: "node:childless.hide-label",
        style: { "text-opacity": 0 },
      },

      // ── Hovered node ──
      {
        selector: "node:childless.hovered",
        style: {
          "border-width": 3,
          "border-opacity": 0.6,
          "background-opacity": 1,
          "font-weight": "600",
          color: textPrimary,
        },
      },

      // ── Selected node ──
      {
        selector: "node:childless:selected",
        style: {
          "border-width": 3,
          "border-color": brand,
          "border-opacity": 1,
          "background-opacity": 1,
          "shadow-blur": 15,
          "shadow-color": brand,
          "shadow-opacity": 0.35,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
          "font-weight": "600",
          "font-size": 12,
          color: textPrimary,
        },
      },

      // ── Edges — thin, subtle, curved ──
      {
        selector: "edge",
        style: {
          width: 1,
          "line-color": edgeColor,
          "line-opacity": light ? 0.5 : 0.35,
          "target-arrow-color": edgeColor,
          "target-arrow-shape": "triangle",
          "arrow-scale": 0.4,
          "curve-style": "bezier",
          "transition-property": "line-color, width, opacity",
          "transition-duration": 180,
        },
      },

      // ── Highlighted edges (neighbor of hovered/selected) ──
      {
        selector: "edge.highlighted",
        style: {
          "line-color": "data(sourceColor)",
          "line-opacity": 0.7,
          "target-arrow-color": "data(sourceColor)",
          width: 2,
        },
      },

      // ── Dimmed (non-neighbors during hover) ──
      {
        selector: "node.dimmed",
        style: {
          opacity: 0.12,
          "text-opacity": 0,
        },
      },
      {
        selector: "edge.dimmed",
        style: {
          opacity: 0.05,
        },
      },

      // ── Hub pulse during progressive reveal ──
      {
        selector: "node:childless.hub-pulse",
        style: {
          "border-width": 4,
          "border-color": brand,
          "border-opacity": 0.6,
          "shadow-blur": 20,
          "shadow-color": brand,
          "shadow-opacity": 0.4,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
        },
      },

      // ── Cola drag halo ──
      {
        selector: "node:childless.cola-dragging",
        style: {
          "shadow-blur": 15,
          "shadow-color": brand,
          "shadow-opacity": 0.35,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
        },
      },

      // ── Cola neighbor glow ──
      {
        selector: "node:childless.cola-neighbor",
        style: {
          "border-width": 2.5,
          "border-opacity": 0.5,
          "border-color": brand,
        },
      },
    ];

    return { stylesheet, isLight: light };
  }, [themeKey]);
}
