import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { formatHumanDuration } from "@shared/lib/dates";
import { getCategoryColor } from "@shared/lib/productivity";
import type { ActivityCategory, TrackedApp } from "@shared/types";
import { Search } from "lucide-react";
import { useMemo, useState } from "react";

interface TrackedAppsListProps {
  apps: TrackedApp[];
  categories: ActivityCategory[];
  onReassigned: () => void;
}

function appKey(app: TrackedApp): string {
  return `${app.appName}:${app.siteName ?? ""}`;
}

export function TrackedAppsList({ apps, categories, onReassigned }: TrackedAppsListProps) {
  const [search, setSearch] = useState("");
  const [showUncategorized, setShowUncategorized] = useState(false);
  const [editingKey, setEditingKey] = useState<string | null>(null);

  const reassignMut = useMutation("productivity_category_upsert");

  const { filtered, uncategorizedCount } = useMemo(() => {
    let result = apps;
    let uncategorized = 0;
    for (const a of apps) {
      if (!a.categoryId) uncategorized++;
    }
    if (showUncategorized) {
      result = result.filter((a) => !a.categoryId);
    }
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(
        (a) => a.displayName.toLowerCase().includes(q) || a.appName.toLowerCase().includes(q),
      );
    }
    return { filtered: result, uncategorizedCount: uncategorized };
  }, [apps, search, showUncategorized]);

  const categoryColorMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const cat of categories) {
      map.set(cat.id, cat.color ?? getCategoryColor(cat.id));
    }
    return map;
  }, [categories]);

  const handleReassign = async (app: TrackedApp, newCategoryId: string) => {
    const newCat = categories.find((c) => c.id === newCategoryId);
    if (!newCat) return;

    // Step 1: Remove the app/site pattern from the OLD category (if any)
    if (app.categoryId && app.categoryId !== newCategoryId) {
      const oldCat = categories.find((c) => c.id === app.categoryId);
      if (oldCat?.rules) {
        const oldRules = oldCat.rules;
        const cleanedRules = app.siteName
          ? {
              appNames: oldRules.appNames,
              bundleIds: oldRules.bundleIds,
              urlPatterns: oldRules.urlPatterns.filter((p) => p !== app.siteName),
            }
          : {
              appNames: oldRules.appNames.filter((n) => n !== app.appName),
              bundleIds: oldRules.bundleIds,
              urlPatterns: oldRules.urlPatterns,
            };
        await reassignMut.mutate({
          id: oldCat.id,
          name: oldCat.name,
          category_type: oldCat.categoryType,
          color: oldCat.color,
          icon: null,
          rules: cleanedRules,
        });
      }
    }

    // Step 2: Add the app/site pattern to the NEW category
    const rules = newCat.rules ?? { appNames: [], bundleIds: [], urlPatterns: [] };
    const updatedRules = app.siteName
      ? {
          appNames: rules.appNames,
          bundleIds: rules.bundleIds,
          urlPatterns: [...new Set([...rules.urlPatterns, app.siteName])],
        }
      : {
          appNames: [...new Set([...rules.appNames, app.appName])],
          bundleIds: rules.bundleIds,
          urlPatterns: rules.urlPatterns,
        };

    await reassignMut.mutate({
      id: newCat.id,
      name: newCat.name,
      category_type: newCat.categoryType,
      color: newCat.color,
      icon: null,
      rules: updatedRules,
    });

    // Step 3: Re-categorize historical events for this app/site
    try {
      await ipc("productivity_recategorize_app", {
        app_name: app.appName,
        site_name: app.siteName ?? null,
        new_category_id: newCategoryId,
      });
    } catch (e) {
      // Tolerate missing command (not yet deployed) — new events will still be categorized correctly.
      // Log other errors so storage/network failures are visible.
      const msg = e instanceof Error ? e.message : String(e);
      if (!msg.includes("unknown command") && !msg.includes("not found")) {
        console.warn("recategorize_app failed:", msg);
      }
    }

    setEditingKey(null);
    onReassigned();
  };

  return (
    <div className="glass-card p-3 flex flex-col gap-2">
      <h3 className="text-xs font-medium text-muted-foreground">Tracked Apps & Sites</h3>

      {/* Search */}
      <div className="relative">
        <Search
          size={12}
          className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground"
        />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search..."
          className="glass-input w-full pl-7 pr-2.5 py-1.5 text-[11px] rounded-lg"
        />
      </div>

      {/* Filter toggle */}
      {uncategorizedCount > 0 && (
        <button
          type="button"
          onClick={() => setShowUncategorized(!showUncategorized)}
          className={`text-2xs font-light px-2 py-1 rounded-lg transition-colors ${
            showUncategorized ? "bg-brand/20 text-brand" : "text-muted-foreground hover:bg-card"
          }`}
        >
          Uncategorized ({uncategorizedCount})
        </button>
      )}

      {/* App list */}
      <div className="flex flex-col gap-0.5 max-h-[600px] overflow-y-auto">
        {filtered.map((app) => {
          const key = appKey(app);
          return (
            <TrackedAppRow
              key={key}
              app={app}
              color={app.categoryId ? (categoryColorMap.get(app.categoryId) ?? null) : null}
              categories={categories}
              isEditing={editingKey === key}
              onEdit={() => setEditingKey(key)}
              onReassign={(catId) => handleReassign(app, catId)}
              onCancel={() => setEditingKey(null)}
            />
          );
        })}
        {filtered.length === 0 && (
          <p className="text-[11px] font-light text-dim py-4 text-center">No apps found</p>
        )}
      </div>
    </div>
  );
}

function TrackedAppRow({
  app,
  color,
  categories,
  isEditing,
  onEdit,
  onReassign,
  onCancel,
}: {
  app: TrackedApp;
  color: string | null;
  categories: ActivityCategory[];
  isEditing: boolean;
  onEdit: () => void;
  onReassign: (categoryId: string) => void;
  onCancel: () => void;
}) {
  return (
    <div className="flex items-center gap-2 px-1.5 py-1 rounded-md hover:bg-card group">
      {color ? (
        <span className="size-2 rounded-sm flex-shrink-0" style={{ backgroundColor: color }} />
      ) : (
        <span className="size-2 rounded-sm flex-shrink-0 border border-dashed border-muted" />
      )}
      <div className="flex-1 min-w-0">
        <div className="text-[11px] font-light text-foreground truncate">{app.displayName}</div>
        <div className="text-[9px] font-light text-dim">
          {app.categoryName ?? "Uncategorized"} · {formatHumanDuration(app.totalSecs)}
        </div>
      </div>
      {isEditing ? (
        <select
          className="glass-input text-2xs px-1.5 py-0.5 rounded-md w-24"
          defaultValue={app.categoryId ?? ""}
          onChange={(e) => onReassign(e.target.value)}
          onBlur={onCancel}
        >
          <option value="" disabled>
            Pick...
          </option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      ) : (
        <button
          type="button"
          onClick={onEdit}
          className="text-[9px] font-light text-dim opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground"
        >
          Edit
        </button>
      )}
    </div>
  );
}
