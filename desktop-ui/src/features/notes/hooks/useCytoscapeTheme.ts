import type { Stylesheet } from "cytoscape";
import { useEffect, useMemo, useState } from "react";

function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function isLightTheme(): boolean {
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
    const light = isLightTheme();
    const textPrimary = getCssVar("--text-primary") || (light ? "#000000" : "#f0f2f5");
    const textSecondary = getCssVar("--text-secondary") || (light ? "#525252" : "#c8cdd4");
    const textMuted = getCssVar("--text-muted") || (light ? "#737373" : "#7d8590");
    const border = getCssVar("--border") || (light ? "#d4d4d4" : "rgba(255,255,255,0.1)");
    const edgeColor = light ? "#d4d4d4" : "rgba(255,255,255,0.12)";
    const brand = getCssVar("--brand") || (light ? "#ca8a04" : "#f97316");

    const stylesheet: Stylesheet[] = [
      // ── Compound parents — invisible (clustering via node color + legend) ──
      {
        selector: "node:parent",
        style: {
          "background-opacity": 0,
          "border-width": 0,
          label: "",
          padding: "12px",
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
    ];

    return { stylesheet, isLight: light };
  }, [themeKey]);
}
