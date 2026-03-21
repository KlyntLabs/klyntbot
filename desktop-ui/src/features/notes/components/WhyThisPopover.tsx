import type { ReactNode } from "react";

interface WhyThisPopoverProps {
  sourceContext?: string | null;
  domain: string;
  children: ReactNode;
}

export function WhyThisPopover({ sourceContext, domain, children }: WhyThisPopoverProps) {
  return (
    <div className="group relative">
      {children}
      <div className="absolute bottom-full left-0 mb-1 hidden group-hover:block z-50 w-56 rounded-lg glass-panel p-2.5 shadow-xl">
        <p className="text-[10px] text-muted mb-1">Why this was suggested</p>
        <p className="text-xs text-primary">{domain}</p>
        {sourceContext && (
          <p className="text-[10px] text-muted mt-1 line-clamp-3">"{sourceContext}"</p>
        )}
      </div>
    </div>
  );
}
