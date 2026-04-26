import { ChevronDown } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";

interface CollapsibleSectionProps {
  title: string;
  icon: ReactNode;
  storageKey: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function CollapsibleSection({
  title,
  icon,
  storageKey,
  defaultOpen = false,
  children,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(() => {
    const stored = localStorage.getItem(storageKey);
    return stored !== null ? stored === "true" : defaultOpen;
  });

  const isInitial = useRef(true);
  useEffect(() => {
    if (isInitial.current) {
      isInitial.current = false;
      return;
    }
    localStorage.setItem(storageKey, String(open));
  }, [storageKey, open]);

  const toggle = useCallback(() => setOpen((o) => !o), []);

  return (
    <div>
      <button
        type="button"
        onClick={toggle}
        className="w-full flex items-center gap-2 px-1 py-2 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
      >
        <span className="shrink-0">{icon}</span>
        {title}
        <ChevronDown
          size={14}
          strokeWidth={1.5}
          className={`ml-auto transition-transform duration-200 ${open ? "rotate-180" : ""}`}
        />
      </button>
      <div
        className={`grid transition-[grid-template-rows] duration-300 ease-out ${
          open ? "grid-rows-[1fr]" : "grid-rows-[0fr]"
        }`}
      >
        <div className="overflow-hidden">{children}</div>
      </div>
    </div>
  );
}
