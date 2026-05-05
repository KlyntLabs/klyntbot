import { useEffect } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

export function PluginsView() {
  useEffect(() => {
    import("@/styles/plugins.css");
  }, []);

  return (
    <section className="plugins-view">
      <div className="plugins-view__pane" data-testid="plugins-active-pane" data-plugin="coding-memory">
        <CodingMemoryPlugin />
      </div>
    </section>
  );
}
