import { useEffect } from "react";

export function PluginsView() {
  useEffect(() => {
    import("@/styles/plugins.css");
  }, []);

  return (
    <section className="plugins-view">
      <div className="plugins-view__pane" data-testid="plugins-active-pane">
        <p className="plugins-view__empty">No plugins installed.</p>
      </div>
    </section>
  );
}
