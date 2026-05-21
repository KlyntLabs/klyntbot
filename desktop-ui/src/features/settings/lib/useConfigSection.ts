import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@/api/client";

export function useConfigSection<T extends object>(section: string) {
  const [value, setValue] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [patching, setPatching] = useState(false);
  const patchGenRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = (await invoke("config_get_section", { section })) as T;
        if (!cancelled) setValue(data);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [section]);

  const patch = useCallback(
    async (p: Partial<T>) => {
      // No-op guard: skip if every key in the patch equals the current value
      if (
        value &&
        Object.entries(p).every(([k, v]) => (value as Record<string, unknown>)[k] === v)
      ) {
        return value;
      }

      const gen = ++patchGenRef.current;
      setPatching(true);
      setError(null);
      try {
        const updated = (await invoke("config_update_section", {
          section,
          patch: p,
        })) as T;
        // Only apply the update if this is still the most recent patch
        if (gen === patchGenRef.current) {
          setValue(updated);
        }
        return updated;
      } catch (e) {
        if (gen === patchGenRef.current) {
          setError(String(e));
        }
        throw e;
      } finally {
        if (gen === patchGenRef.current) {
          setPatching(false);
        }
      }
    },
    [section, value],
  );

  return { value, loading, error, patching, patch, setValue };
}
