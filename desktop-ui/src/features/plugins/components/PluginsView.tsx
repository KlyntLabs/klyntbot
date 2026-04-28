import { useState } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

type PluginId = "coding-memory" | "skills" | "mcp" | "klynt-cli";

const PLUGIN_TABS: ReadonlyArray<{ id: PluginId; label: string; available: boolean }> = [
  { id: "coding-memory", label: "Coding Memory", available: true },
  { id: "skills", label: "Skills", available: false },
  { id: "mcp", label: "MCP Servers", available: false },
  { id: "klynt-cli", label: "Klynt CLI", available: false },
];

export function PluginsView() {
  const [active, setActive] = useState<PluginId>("coding-memory");

  return (
    <section className="plugins-view" aria-labelledby="plugins-heading">
      <header className="plugins-view__header">
        <h1 id="plugins-heading" className="plugins-view__title">Plugins</h1>
        <p className="plugins-view__subtitle">Internal plugins for inspecting and tuning your Klynt platform.</p>
      </header>
      <div role="tablist" aria-label="Plugins" className="plugins-view__tabs">
        {PLUGIN_TABS.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            type="button"
            aria-selected={active === tab.id}
            disabled={!tab.available}
            className={
              "plugins-view__tab" +
              (active === tab.id ? " plugins-view__tab--active" : "") +
              (tab.available ? "" : " plugins-view__tab--coming-soon")
            }
            onClick={() => tab.available && setActive(tab.id)}
          >
            {tab.label}
            {!tab.available && <span className="plugins-view__tab-badge">Soon</span>}
          </button>
        ))}
      </div>
      <div className="plugins-view__pane" data-testid="plugins-active-pane" data-plugin={active}>
        {active === "coding-memory" ? <CodingMemoryPlugin /> : null}
      </div>
    </section>
  );
}
