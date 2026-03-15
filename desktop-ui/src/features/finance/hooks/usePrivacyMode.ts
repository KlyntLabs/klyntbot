import { useCallback, useState } from "react";

const STORAGE_KEY = "finance:privacyMode";

export function usePrivacyMode() {
  const [hidden, setHidden] = useState<boolean>(() => {
    return localStorage.getItem(STORAGE_KEY) === "true";
  });

  const toggle = useCallback(() => {
    setHidden((prev) => {
      const next = !prev;
      localStorage.setItem(STORAGE_KEY, String(next));
      return next;
    });
  }, []);

  return { hidden, toggle };
}
