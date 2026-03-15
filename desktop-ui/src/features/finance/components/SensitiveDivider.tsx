import { Lock } from "lucide-react";

export function SensitiveDivider() {
  return (
    <div className="flex items-center gap-3 my-6 px-1" aria-label="Sensitive financial data below">
      <div className="flex-1 h-px bg-gradient-to-r from-transparent via-white/[0.08] to-transparent" />
      <div className="flex items-center gap-1.5 text-[9px] text-dim uppercase tracking-widest">
        <Lock className="w-3 h-3" strokeWidth={1.5} />
        <span>Amounts & Balances</span>
      </div>
      <div className="flex-1 h-px bg-gradient-to-r from-transparent via-white/[0.08] to-transparent" />
    </div>
  );
}
