import { Eye, EyeOff } from 'lucide-react';

interface TransparencyToggleProps {
  enabled: boolean;
  onToggle: () => void;
}

export function TransparencyToggle({ enabled, onToggle }: TransparencyToggleProps) {
  const Icon = enabled ? Eye : EyeOff;

  return (
    <button
      onClick={onToggle}
      className={`w-8 h-8 flex items-center justify-center rounded-lg transition-colors ${
        enabled
          ? 'bg-brand/10 text-brand hover:bg-brand/20'
          : 'text-muted hover:bg-surface-base hover:text-secondary'
      }`}
      title={enabled ? 'Hide transparency data' : 'Show transparency data'}
    >
      <Icon className="w-4 h-4" strokeWidth={1.5} />
    </button>
  );
}
