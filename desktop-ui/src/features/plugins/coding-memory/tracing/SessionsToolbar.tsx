export type SortKey = "recent" | "created" | "size" | "errors";

interface Props {
  sessionCount: number;
  search: string;
  onSearchChange: (s: string) => void;
  sort: SortKey;
  onSortChange: (s: SortKey) => void;
  importedOnly: boolean;
  onImportedToggle: () => void;
  group: boolean;
  onGroupToggle: () => void;
  layout: "grid" | "list";
  onLayoutToggle: () => void;
  onImport: () => void;
}

export function SessionsToolbar(p: Props) {
  return (
    <div className="tracing-toolbar">
      <div className="tracing-toolbar__search">
        <input
          type="search"
          placeholder="Search sessions…"
          value={p.search}
          onChange={(e) => p.onSearchChange(e.target.value)}
        />
      </div>
      <select
        className="tracing-toolbar__sort"
        value={p.sort}
        onChange={(e) => p.onSortChange(e.target.value as SortKey)}
      >
        <option value="recent">Recent</option>
        <option value="created">Created</option>
        <option value="size">Size</option>
        <option value="errors">Errors</option>
      </select>
      <button
        type="button"
        className={"tracing-toolbar__btn" + (p.importedOnly ? " tracing-toolbar__btn--on" : "")}
        onClick={p.onImportedToggle}
      >
        Imported
      </button>
      <button
        type="button"
        className={"tracing-toolbar__btn" + (p.group ? " tracing-toolbar__btn--on" : "")}
        onClick={p.onGroupToggle}
      >
        Group
      </button>
      <button
        type="button"
        className="tracing-toolbar__btn"
        onClick={p.onLayoutToggle}
        title={p.layout === "grid" ? "Switch to list" : "Switch to grid"}
      >
        {p.layout === "grid" ? "List" : "Grid"}
      </button>
      <button type="button" className="tracing-toolbar__btn" onClick={p.onImport}>
        Import
      </button>
      <span className="tracing-toolbar__count">{p.sessionCount} sessions</span>
    </div>
  );
}
