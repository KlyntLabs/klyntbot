import { Atom, Brain, Network, Search, TreePine } from "lucide-react";
import type { SmartView } from "../hooks/useGraphData";

type LayerKey = "communities" | "entities" | "tree";

interface GraphToolbarProps {
  view: SmartView;
  onViewChange: (view: SmartView) => void;
  hopRadius: number;
  onHopRadiusChange: (r: number) => void;
  searchQuery: string;
  onSearchChange: (q: string) => void;
  clusteringMode: "notebook" | "semantic";
  onClusteringModeChange: (mode: "notebook" | "semantic") => void;
  renderMode: "2d" | "3d";
  onRenderModeChange: (mode: "2d" | "3d") => void;
  layerState: Record<LayerKey, boolean>;
  onLayerToggle: (layer: LayerKey) => void;
}

const VIEW_OPTIONS: { value: SmartView; label: string }[] = [
  { value: "local", label: "Local" },
  { value: "full", label: "Full" },
  { value: "by-tag", label: "By Tag" },
  { value: "by-notebook", label: "By Notebook" },
  { value: "orphans", label: "Orphans" },
];

const CLUSTERING_OPTIONS: { value: "notebook" | "semantic"; label: string }[] = [
  { value: "notebook", label: "Notebook" },
  { value: "semantic", label: "Semantic" },
];

const LAYER_OPTIONS: {
  key: LayerKey;
  label: string;
  Icon: typeof Network;
}[] = [
  { key: "communities", label: "Communities", Icon: Network },
  { key: "entities", label: "Entities", Icon: Atom },
  { key: "tree", label: "Tree", Icon: TreePine },
];

export function GraphToolbar({
  view,
  onViewChange,
  hopRadius,
  onHopRadiusChange,
  searchQuery,
  onSearchChange,
  clusteringMode,
  onClusteringModeChange,
  renderMode,
  onRenderModeChange,
  layerState,
  onLayerToggle,
}: GraphToolbarProps) {
  const handleSemanticClick = () => {
    if (!layerState.communities) onLayerToggle("communities");
    if (!layerState.entities) onLayerToggle("entities");
    onClusteringModeChange("semantic");
  };
  return (
    <div className="flex items-center gap-2 px-3 py-2 shrink-0">
      {/* Smart view pills */}
      <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
        {VIEW_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onViewChange(opt.value)}
            className={`px-2.5 py-1 text-xs rounded-md transition-all ${
              view === opt.value
                ? "bg-brand/20 text-brand font-medium shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`}
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Hop radius selector (only for local view) */}
      {view === "local" && (
        <div className="flex items-center gap-1 text-xs text-muted-foreground">
          <span>Hops:</span>
          <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
            {[1, 2, 3].map((r) => (
              <button
                key={r}
                type="button"
                onClick={() => onHopRadiusChange(r)}
                className={`size-6 rounded-md text-xs flex items-center justify-center transition-all ${
                  hopRadius === r
                    ? "bg-brand/20 text-brand font-medium"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent"
                }`}
              >
                {r}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Clustering mode switcher */}
      <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
        {CLUSTERING_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={
              opt.value === "semantic"
                ? handleSemanticClick
                : () => onClusteringModeChange(opt.value)
            }
            className={`px-2 py-1 text-xs rounded-md transition-all ${
              clusteringMode === opt.value
                ? "bg-brand/20 text-brand font-medium shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-accent"
            }`}
            title={
              opt.value === "semantic"
                ? "Switch to semantic clustering with communities and entities"
                : undefined
            }
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* Layer toggles */}
      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        <span>Layers:</span>
        <div className="flex items-center gap-0.5 bg-card rounded-lg p-0.5">
          {LAYER_OPTIONS.map(({ key, label, Icon }) => (
            <button
              key={key}
              type="button"
              onClick={() => onLayerToggle(key)}
              className={`px-2 py-1 text-xs rounded-md transition-all flex items-center gap-1 ${
                layerState[key]
                  ? "bg-brand/20 text-brand font-medium shadow-sm"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent"
              }`}
              title={`Toggle ${label} layer`}
            >
              <Icon size={12} />
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Brain View toggle */}
      <button
        type="button"
        onClick={() => onRenderModeChange(renderMode === "2d" ? "3d" : "2d")}
        className={`flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-lg transition-all ${
          renderMode === "3d"
            ? "bg-brand/20 text-brand font-medium shadow-sm"
            : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }`}
        title={renderMode === "3d" ? "Exit Brain View" : "Enter Brain View"}
      >
        <Brain size={14} />
        {renderMode === "3d" ? "Exit Brain View" : "Brain View"}
      </button>

      {/* Search input */}
      <div className="relative">
        <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-dim pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Filter nodes..."
          className="w-40 pl-7 pr-2 py-1 text-xs rounded-lg bg-card border border-border-subtle text-foreground placeholder:text-dim outline-none focus:border-brand/40 transition-colors"
        />
      </div>
    </div>
  );
}
