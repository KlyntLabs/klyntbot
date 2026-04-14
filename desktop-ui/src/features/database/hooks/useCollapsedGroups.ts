import { useUpdateView } from "@features/database/hooks/useViews";
import type { ViewDefinition } from "@shared/types";
import { useCallback, useEffect, useRef, useState } from "react";

const PERSIST_DEBOUNCE_MS = 400;

export function useCollapsedGroups(databaseId: string, view: ViewDefinition) {
  const initial = view.config.collapsedGroups ?? [];
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set(initial));
  const { mutate: updateView } = useUpdateView(databaseId);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latest = useRef<string[]>(initial);
  const configRef = useRef(view.config);
  configRef.current = view.config;

  const persist = useCallback(
    (next: string[]) => {
      latest.current = next;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        void updateView(view.id, {
          config: { ...configRef.current, collapsedGroups: latest.current },
        });
      }, PERSIST_DEBOUNCE_MS);
    },
    [updateView, view.id],
  );

  useEffect(() => {
    setCollapsed(new Set(view.config.collapsedGroups ?? []));
  }, [view.id]);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  const toggle = useCallback(
    (key: string) => {
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        persist([...next]);
        return next;
      });
    },
    [persist],
  );

  return { collapsed, toggle };
}
