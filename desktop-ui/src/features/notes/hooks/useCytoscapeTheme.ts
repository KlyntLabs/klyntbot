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
    const textMuted = getCssVar("--text-muted") || (light ? "#737373" : "#7d8590");
    const border = getCssVar("--border") || (light ? "#e5e5e5" : "rgba(255,255,255,0.08)");
    const brand = getCssVar("--brand") || (light ? "#ca8a04" : "#f97316");

    const stylesheet: Stylesheet[] = [
      {
        selector: "node:parent",
        style: {
          "background-opacity": 0.06,
          "border-width": 1,
          "border-color": border,
          "border-opacity": 0.5,
          label: "data(label)",
          "font-size": 11,
          "font-weight": "600",
          color: textMuted,
          "text-valign": "top",
          "text-halign": "center",
          "text-margin-y": -6,
          padding: "24px",
          shape: "roundrectangle",
          "corner-radius": light ? 0 : 12,
        },
      },
      {
        selector: "node:childless",
        style: {
          label: "data(label)",
          width: "data(size)",
          height: "data(size)",
          "background-color": "data(color)",
          "border-width": 1.5,
          "border-color": "data(color)",
          "border-opacity": 0.35,
          "font-size": 10,
          "font-weight": "500",
          color: textPrimary,
          "text-valign": "bottom",
          "text-halign": "center",
          "text-margin-y": 4,
          "text-max-width": "80px",
          "text-wrap": "ellipsis",
          "min-zoomed-font-size": 0,
          "text-opacity": 1,
        },
      },
      {
        selector: "node:childless.hide-label",
        style: { "text-opacity": 0 },
      },
      {
        selector: "node:childless:selected",
        style: {
          "border-width": 3,
          "border-color": brand,
          "shadow-blur": 12,
          "shadow-color": brand,
          "shadow-opacity": 0.4,
          "shadow-offset-x": 0,
          "shadow-offset-y": 0,
          "font-weight": "600",
          "font-size": 11,
        },
      },
      {
        selector: "edge",
        style: {
          width: "data(weight)",
          "line-color": border,
          "target-arrow-color": border,
          "target-arrow-shape": "triangle",
          "arrow-scale": 0.5,
          "curve-style": "bezier",
          opacity: 0.6,
        },
      },
      {
        selector: "edge.highlighted",
        style: {
          "line-color": "data(sourceColor)",
          "target-arrow-color": "data(sourceColor)",
          width: 2.5,
          opacity: 0.8,
        },
      },
      {
        selector: "node.dimmed",
        style: { opacity: 0.15 },
      },
      {
        selector: "edge.dimmed",
        style: { opacity: 0.08 },
      },
      {
        selector: 'node:parent[type="orphan-linked"]',
        style: { "background-color": "#9CA3AF" },
      },
      {
        selector: 'node:parent[type="orphan-isolated"]',
        style: { "background-color": "#6B7280" },
      },
    ];

    return { stylesheet, isLight: light };
  }, [themeKey]);
}
