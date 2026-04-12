import type { FieldDefinition } from "@shared/types";

interface FieldEditorProps {
  field: FieldDefinition;
  value: unknown;
  onChange: (value: unknown) => void;
}

export function FieldEditor({ field, value, onChange }: FieldEditorProps) {
  switch (field.fieldType) {
    case "text":
    case "email":
    case "phone":
    case "person":
    case "url":
      return (
        <input
          type={field.fieldType === "email" ? "email" : field.fieldType === "url" ? "url" : "text"}
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
          placeholder={field.name}
        />
      );

    case "number":
      return (
        <input
          type="number"
          value={value != null ? Number(value) : ""}
          onChange={(e) => onChange(e.target.value ? Number(e.target.value) : null)}
          className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm tabular-nums outline-none focus:border-accent"
          placeholder={field.name}
        />
      );

    case "checkbox":
      return (
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={Boolean(value)}
            onChange={(e) => onChange(e.target.checked)}
            className="rounded border-border"
          />
          <span className="text-sm">{field.name}</span>
        </label>
      );

    case "select":
      return <SelectEditor field={field} value={value} onChange={onChange} />;

    case "multi_select":
      return <MultiSelectEditor field={field} value={value} onChange={onChange} />;

    case "date":
      return (
        <input
          type="date"
          value={dateToInputValue(value)}
          onChange={(e) => onChange(e.target.value ? new Date(e.target.value).toISOString() : null)}
          className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
        />
      );

    case "relation":
    case "rollup":
    case "formula":
    case "created_time":
    case "last_edited":
    case "files":
      return (
        <span className="text-muted text-sm italic">
          {field.fieldType === "created_time" || field.fieldType === "last_edited"
            ? "Auto-managed"
            : `${field.fieldType} (not editable inline)`}
        </span>
      );

    default:
      return (
        <input
          type="text"
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
          placeholder={field.name}
        />
      );
  }
}

function SelectEditor({
  field,
  value,
  onChange,
}: {
  field: FieldDefinition;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const options: string[] = Array.isArray(field.options) ? field.options : [];
  return (
    <select
      value={String(value ?? "")}
      onChange={(e) => onChange(e.target.value || null)}
      className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
    >
      <option value="">—</option>
      {options.map((opt) => (
        <option key={opt} value={opt}>
          {opt}
        </option>
      ))}
    </select>
  );
}

function MultiSelectEditor({
  field,
  value,
  onChange,
}: {
  field: FieldDefinition;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const options: string[] = Array.isArray(field.options) ? field.options : [];
  const selected: string[] = Array.isArray(value) ? value : [];

  const toggle = (opt: string) => {
    const next = selected.includes(opt) ? selected.filter((s) => s !== opt) : [...selected, opt];
    onChange(next);
  };

  return (
    <div className="flex flex-wrap gap-1">
      {options.map((opt) => (
        <button
          key={opt}
          type="button"
          onClick={() => toggle(opt)}
          className={`rounded-full px-2 py-0.5 text-xs font-medium transition-colors ${
            selected.includes(opt)
              ? "bg-accent text-white"
              : "bg-surface-raised text-muted hover:bg-surface-hover"
          }`}
        >
          {opt}
        </button>
      ))}
    </div>
  );
}

function dateToInputValue(value: unknown): string {
  if (!value) return "";
  try {
    return new Date(String(value)).toISOString().split("T")[0];
  } catch {
    return "";
  }
}
