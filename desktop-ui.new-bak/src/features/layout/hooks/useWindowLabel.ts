import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

export function useWindowLabel(defaultLabel = "main") {
  const [label, setLabel] = useState(defaultLabel);

  useEffect(() => {
    try {
      const window = getCurrentWindow();
      setLabel(window.label ?? defaultLabel);
    } catch {
      setLabel(defaultLabel);
    }
  }, [defaultLabel]);

  return label;
}
