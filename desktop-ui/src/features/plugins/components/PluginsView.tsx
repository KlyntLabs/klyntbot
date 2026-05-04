import { useState } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

type PluginId = "coding-memory" | "skills" | "mcp";

const PLUGIN_TABS: ReadonlyArray<{ id: PluginId; label: string; available: boolean }> = [
  { id: "coding-memory", label: "Coding Memory", available: true },
  { id: "skills", label: "Skills", available: false },
  { id: "mcp", label: "MCP Servers", available: false },
];

export function PluginsView() {
  const [active, setActive] = useState<PluginId>("coding-memory");

  return (
    <section className="plugins-view" aria-labelledby="plugins-heading">
      <header className="plugins-view__header">
        <h1 id="plugins-heading" className="plugins-view__title">
          Plugins
        </h1>
        <p className="plugins-view__subtitle">
          Internal plugins for inspecting and tuning your Klynt platform.
        </p>
      </header>
      <div className="plugins-view__tabs-wrap">
        <div
          className="panel-tabs"
          role="tablist"
          aria-label="Plugins"
          aria-orientation="horizontal"
        >
          {PLUGIN_TABS.map((tab) => {
            const isActive = active === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={isActive}
                disabled={!tab.available}
                className={`panel-tab panel-tab--text${isActive ? " is-active" : ""}`}
                onClick={() => tab.available && setActive(tab.id)}
              >
                {tab.label}
                {!tab.available && <span className="panel-tab__badge">Soon</span>}
              </button>
            );
          })}
        </div>
      </div>
      <div className="plugins-view__pane" data-testid="plugins-active-pane" data-plugin={active}>
        {active === "coding-memory" ? <CodingMemoryPlugin /> : null}
      </div>
    </section>
  );
}
