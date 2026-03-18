import { ChevronRight } from "lucide-react";
import { forwardRef, type ReactNode, useEffect, useRef } from "react";
import { createPortal } from "react-dom";

// ── Menu Root ───────────────────────────────────────────────────────────

interface ContextMenuProps {
  x: number;
  y: number;
  children: ReactNode;
  onClose: () => void;
}

export const ContextMenu = forwardRef<HTMLDivElement, ContextMenuProps>(function ContextMenu(
  { x, y, children, onClose },
  ref,
) {
  const innerRef = useRef<HTMLDivElement>(null);
  const menuRef = (ref as React.RefObject<HTMLDivElement>) ?? innerRef;

  // Keyboard navigation
  useEffect(() => {
    const el = menuRef.current;
    if (!el) return;

    // Focus first item
    const first = el.querySelector<HTMLElement>("[role=menuitem]");
    first?.focus();

    const handler = (e: KeyboardEvent) => {
      const items = el.querySelectorAll<HTMLElement>("[role=menuitem]");
      if (!items.length) return;
      const arr = Array.from(items);
      const idx = arr.indexOf(document.activeElement as HTMLElement);

      if (e.key === "ArrowDown") {
        e.preventDefault();
        arr[(idx + 1) % arr.length]?.focus();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        arr[(idx - 1 + arr.length) % arr.length]?.focus();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };

    el.addEventListener("keydown", handler);
    return () => el.removeEventListener("keydown", handler);
  }, [menuRef, onClose]);

  // Keep menu within viewport
  const menuX = Math.min(x, window.innerWidth - 220);
  const menuY = Math.min(y, window.innerHeight - 300);

  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      onMouseDown={(e) => e.stopPropagation()}
      className="context-menu fixed z-50 py-[5px] min-w-[200px] animate-[menu-appear_120ms_ease-out]"
      style={{ left: menuX, top: menuY }}
    >
      {children}
    </div>,
    document.body,
  );
});

// ── Menu Item ───────────────────────────────────────────────────────────

interface ContextMenuItemProps {
  children: ReactNode;
  onClick: () => void;
  destructive?: boolean;
  icon?: ReactNode;
}

export function ContextMenuItem({ children, onClick, destructive, icon }: ContextMenuItemProps) {
  return (
    <button
      type="button"
      role="menuitem"
      tabIndex={-1}
      onClick={onClick}
      className={`w-[calc(100%-10px)] mx-[5px] px-2.5 py-[5px] text-[13px] font-normal rounded-md flex items-center gap-2.5 text-left transition-colors outline-none focus-visible:outline-none ${
        destructive
          ? "text-destructive hover:bg-destructive/[0.12] focus:bg-destructive/[0.12]"
          : "text-muted-foreground hover:bg-muted focus:bg-muted hover:text-foreground focus:text-foreground"
      }`}
    >
      {icon && (
        <span className="w-4 h-4 flex items-center justify-center shrink-0 opacity-70">{icon}</span>
      )}
      <span className="flex-1 truncate">{children}</span>
    </button>
  );
}

// ── Separator ───────────────────────────────────────────────────────────

export function ContextMenuSeparator() {
  return <div className="h-px bg-muted my-[5px] mx-2.5" />;
}

// ── Submenu trigger ─────────────────────────────────────────────────────

interface ContextMenuSubmenuProps {
  label: ReactNode;
  icon?: ReactNode;
  children: ReactNode;
  open: boolean;
  onToggle: () => void;
  /** Override default submenu panel classes (min-w, max-h, etc.) */
  panelClassName?: string;
}

export function ContextMenuSubmenu({
  label,
  icon,
  children,
  open,
  onToggle,
  panelClassName,
}: ContextMenuSubmenuProps) {
  return (
    <div className="relative">
      <button
        type="button"
        role="menuitem"
        tabIndex={-1}
        onClick={onToggle}
        className="w-[calc(100%-10px)] mx-[5px] px-2.5 py-[5px] text-[13px] font-normal rounded-md flex items-center gap-2.5 text-left text-muted-foreground hover:bg-muted focus:bg-muted hover:text-foreground focus:text-foreground transition-colors outline-none focus-visible:outline-none"
      >
        {icon && (
          <span className="w-4 h-4 flex items-center justify-center shrink-0 opacity-70">
            {icon}
          </span>
        )}
        <span className="flex-1 truncate">{label}</span>
        <ChevronRight className="w-3 h-3 opacity-40" />
      </button>
      {open && (
        <div
          className={
            panelClassName ??
            "context-menu absolute left-full top-0 ml-1 py-[5px] min-w-[180px] max-h-52 overflow-y-auto animate-[menu-appear_100ms_ease-out]"
          }
        >
          {children}
        </div>
      )}
    </div>
  );
}
