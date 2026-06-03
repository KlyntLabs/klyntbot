import type { SettingsDomain } from "@settings/registry/settingsDomains";
import { settingsDomains as defaultDomains } from "@settings/registry/settingsDomains";
import { useState } from "react";
import { cn } from "@/utils/cn";

type Props = {
  domains?: SettingsDomain[];
};

export function SettingsShell({ domains = defaultDomains }: Props) {
  const [activeId, setActiveId] = useState<string>(domains[0]?.id ?? "");

  const activeDomain = domains.find((d) => d.id === activeId);

  return (
    <div className="flex h-full">
      <nav className="flex w-[200px] flex-col gap-0.5 border-r border-border-subtle p-3">
        {domains.map((domain) => (
          <button
            key={domain.id}
            type="button"
            onClick={() => setActiveId(domain.id)}
            className={cn(
              "flex items-center gap-2 rounded-md px-3 py-2 text-left text-ui-sm transition-colors",
              activeId === domain.id
                ? "bg-surface-active text-text-strong"
                : "text-text-muted hover:bg-surface-hover hover:text-text-strong",
            )}
          >
            {domain.icon && <span className="shrink-0">{domain.icon}</span>}
            <span className="truncate">{domain.label}</span>
          </button>
        ))}
      </nav>

      <div className="flex-1 min-h-0 overflow-y-auto p-6">
        {activeDomain ? (
          <activeDomain.Component />
        ) : (
          <div className="text-ui-sm text-text-subtle">Select a setting from the sidebar.</div>
        )}
      </div>
    </div>
  );
}
