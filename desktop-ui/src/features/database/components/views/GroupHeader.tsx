import { ChevronRight } from "lucide-react";

interface Props {
  label: string;
  count: number;
  collapsed: boolean;
  onToggle: () => void;
}

export function GroupHeader({ label, count, collapsed, onToggle }: Props) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className="flex w-full items-center gap-2 bg-surface-base px-3 py-1.5 text-[12px] font-medium text-foreground/70 hover:text-foreground"
    >
      <ChevronRight size={14} className={`transition-transform ${collapsed ? "" : "rotate-90"}`} />
      <span>{label}</span>
      <span className="text-foreground/50">{count}</span>
    </button>
  );
}
