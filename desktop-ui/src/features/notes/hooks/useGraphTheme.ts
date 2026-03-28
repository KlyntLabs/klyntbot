import { useEffect, useState } from "react";

export interface GraphTheme {
  isDark: boolean;
  backgroundColor: string;
  edgeColor: string;
  edgeOpacity: number;
  labelColor: string;
  dimmedOpacity: number;
}

export function useGraphTheme(): GraphTheme {
  const [isDark, setIsDark] = useState(() => {
    return document.documentElement.getAttribute("data-theme") !== "retro";
  });

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.getAttribute("data-theme") !== "retro");
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return {
    isDark,
    backgroundColor: "transparent",
    edgeColor: isDark ? "#4B5563" : "#9CA3AF",
    edgeOpacity: isDark ? 0.35 : 0.5,
    labelColor: isDark ? "rgba(255,255,255,0.7)" : "rgba(0,0,0,0.7)",
    dimmedOpacity: 0.12,
  };
}
