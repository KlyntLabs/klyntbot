import { useMutation } from "@shared/hooks/useMutation";
import type { ActivityCategory } from "@shared/types";
import { Palette, Save, Trash2, X } from "lucide-react";
import { useState } from "react";
import {
  CATEGORY_TYPE_GROUPS,
  DEFAULT_CATEGORY_COLOR,
  getCategoryTypeColor,
} from "../lib/constants";

interface CategoryEditorProps {
  category: ActivityCategory | null;
  onSaved: () => void;
  onDeleted: () => void;
}

const COLOR_SWATCHES = [
  "#22C55E",
  "#10B981",
  "#06B6D4",
  "#2DD4BF",
  "#34D399",
  "#60A5FA",
  "#8B5CF6",
  "#A78BFA",
  "#C084FC",
  "#F59E0B",
  "#FB923C",
  "#94A3B8",
  "#78716C",
  "#A1A1AA",
  "#F43F5E",
  "#EF4444",
  "#E11D48",
  "#FB7185",
  "#F87171",
];

const TYPE_OPTIONS = CATEGORY_TYPE_GROUPS.map((g) => ({
  value: g.type,
  label: `${g.label} (${g.type.charAt(0).toUpperCase()}${g.type.slice(1)})`,
}));

export function CategoryEditor({ category, onSaved, onDeleted }: CategoryEditorProps) {
  const [name, setName] = useState(category?.name ?? "");
  const [type, setType] = useState(category?.categoryType ?? "neutral");
  const [color, setColor] = useState(category?.color ?? DEFAULT_CATEGORY_COLOR);
  const [appNames, setAppNames] = useState<string[]>(category?.rules?.appNames ?? []);
  const [urlPatterns, setUrlPatterns] = useState<string[]>(category?.rules?.urlPatterns ?? []);

  const saveMut = useMutation("productivity_category_upsert");
  const deleteMut = useMutation("productivity_category_delete");

  if (!category) {
    return (
      <div className="glass-card p-6 flex items-center justify-center h-full">
        <p className="text-[13px] font-light text-dim">Select a category to edit</p>
      </div>
    );
  }

  const handleSave = async () => {
    await saveMut.mutate({
      id: category.id,
      name,
      category_type: type,
      color,
      icon: null,
      rules: {
        appNames,
        bundleIds: category.rules?.bundleIds ?? [],
        urlPatterns,
      },
    });
    onSaved();
  };

  const handleDelete = async () => {
    await deleteMut.mutate({ id: category.id });
    onDeleted();
  };

  return (
    <div className="glass-card p-4 flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="w-3 h-3 rounded-sm" style={{ backgroundColor: color }} />
          <h3 className="text-[13px] font-medium text-muted-foreground">Edit Category</h3>
          {category.isSystem && (
            <span className="text-[9px] font-light text-dim bg-accent px-1.5 py-0.5 rounded">
              System
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={handleSave}
            className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg bg-brand/20 text-brand text-[11px] font-medium hover:bg-brand/30 transition-colors"
          >
            <Save size={12} />
            Save
          </button>
          {!category.isSystem && (
            <button
              type="button"
              onClick={handleDelete}
              className="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-destructive/70 text-[11px] hover:bg-destructive/10 transition-colors"
            >
              <Trash2 size={12} />
            </button>
          )}
        </div>
      </div>

      {/* Name */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Name
        </span>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="glass-input px-3 py-1.5 text-[12px] rounded-lg"
        />
      </div>

      {/* Type */}
      <div className="flex flex-col gap-1">
        <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Type
        </span>
        <div className="flex gap-1.5">
          {TYPE_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setType(opt.value)}
              className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-[11px] font-light transition-colors ${
                type === opt.value
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground hover:bg-card"
              }`}
            >
              <span
                className="w-1.5 h-1.5 rounded-full"
                style={{ backgroundColor: getCategoryTypeColor(opt.value) }}
              />
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {/* Color */}
      <div className="flex flex-col gap-1.5">
        <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider flex items-center gap-1">
          <Palette size={10} />
          Color
        </span>
        <div className="flex flex-wrap gap-1.5">
          {COLOR_SWATCHES.map((swatch) => (
            <button
              key={swatch}
              type="button"
              onClick={() => setColor(swatch)}
              className={`w-5 h-5 rounded-md transition-all ${
                color === swatch ? "ring-2 ring-white/40 scale-110" : "hover:scale-105"
              }`}
              style={{ backgroundColor: swatch }}
            />
          ))}
        </div>
      </div>

      <TagListField
        label="App Names"
        placeholder="Add app name..."
        items={appNames}
        onAdd={(item) => setAppNames([...appNames, item])}
        onRemove={(item) => setAppNames(appNames.filter((a) => a !== item))}
      />
      <TagListField
        label="URL / Domain Patterns"
        placeholder="Add domain (e.g. github.com)..."
        items={urlPatterns}
        onAdd={(item) => setUrlPatterns([...urlPatterns, item])}
        onRemove={(item) => setUrlPatterns(urlPatterns.filter((u) => u !== item))}
      />
    </div>
  );
}

function TagListField({
  label,
  placeholder,
  items,
  onAdd,
  onRemove,
}: {
  label: string;
  placeholder: string;
  items: string[];
  onAdd: (item: string) => void;
  onRemove: (item: string) => void;
}) {
  const [value, setValue] = useState("");

  const add = () => {
    const trimmed = value.trim();
    if (trimmed && !items.includes(trimmed)) {
      onAdd(trimmed);
      setValue("");
    }
  };

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
        {label}
      </span>
      <div className="flex flex-wrap gap-1">
        {items.map((item) => (
          <span
            key={item}
            className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-accent text-[11px] font-light text-muted-foreground"
          >
            {item}
            <button
              type="button"
              onClick={() => onRemove(item)}
              className="text-muted-foreground hover:text-destructive"
            >
              <X size={10} />
            </button>
          </span>
        ))}
      </div>
      <div className="flex gap-1">
        <input
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder={placeholder}
          className="glass-input flex-1 px-2.5 py-1 text-[11px] rounded-lg"
        />
        <button
          type="button"
          onClick={add}
          className="px-2 py-1 rounded-lg bg-accent text-[11px] text-muted-foreground hover:text-foreground transition-colors"
        >
          Add
        </button>
      </div>
    </div>
  );
}
