import { createContext, type ReactNode, useContext, useEffect, useState } from "react";

const THEMES = ["light", "dark"] as const;
export type Theme = (typeof THEMES)[number];

interface ThemeContextValue {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  themes: readonly string[];
}

const ThemeContext = createContext<ThemeContextValue>({
  theme: "dark",
  setTheme: () => {},
  themes: THEMES,
});

function readStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem("klynt-theme");
    if (stored === "light" || stored === "dark") return stored;
    // Retired themes map to dark
    if (stored === "retro") return "dark";
  } catch {
    // localStorage unavailable
  }
  return "dark";
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setTheme] = useState<Theme>(readStoredTheme);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem("klynt-theme", theme);
    } catch {
      // localStorage unavailable
    }
  }, [theme]);

  return (
    <ThemeContext.Provider value={{ theme, setTheme, themes: THEMES }}>
      {children}
    </ThemeContext.Provider>
  );
}

export const useTheme = () => useContext(ThemeContext);
