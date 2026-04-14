import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

let mql: MediaQueryList | null = null;
function getMql(): MediaQueryList | null {
  if (mql) return mql;
  if (typeof window === "undefined" || !window.matchMedia) return null;
  mql = window.matchMedia(QUERY);
  return mql;
}

const subscribe = (cb: () => void) => {
  const q = getMql();
  if (!q) return () => {};
  q.addEventListener("change", cb);
  return () => q.removeEventListener("change", cb);
};

const getSnapshot = () => getMql()?.matches ?? false;
const getServerSnapshot = () => false;

export function useReducedMotion(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
