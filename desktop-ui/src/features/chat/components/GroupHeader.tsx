import { ChevronDown } from "lucide-react";

interface GroupHeaderProps {
  groupKey: string;
  label: string;
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>;
  isExpanded: boolean;
  onToggle: (key: string) => void;
}

export function GroupHeader({
  groupKey,
  label,
  icon: Icon,
  isExpanded,
  onToggle,
}: GroupHeaderProps) {
  return (
    <button
      type="button"
      onClick={() => onToggle(groupKey)}
      aria-expanded={isExpanded}
      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-accent transition-colors text-[12px] font-light text-muted-foreground hover:text-foreground"
    >
      <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
      <span className="flex-1 text-left">{label}</span>
      <ChevronDown
        className={`w-3.5 h-3.5 transition-transform ${isExpanded ? "rotate-0" : "-rotate-90"}`}
        strokeWidth={1.5}
      />
    </button>
  );
}
