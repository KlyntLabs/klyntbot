import type { FieldDefinition } from "@shared/types";
import { Input } from "@shared/ui/Input";

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
        <Input
          type={field.fieldType === "email" ? "email" : field.fieldType === "url" ? "url" : "text"}
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.name}
          className="w-full"
        />
      );

    case "number":
      return (
        <Input
          type="number"
          value={value != null ? Number(value) : ""}
          onChange={(e) => onChange(e.target.value ? Number(e.target.value) : null)}
          placeholder={field.name}
          className="w-full tabular-nums"
        />
      );

    case "checkbox":
      return (
        <label className="flex items-center gap-2 cursor-pointer py-0.5">
          <input
            type="checkbox"
            checked={Boolean(value)}
            onChange={(e) => onChange(e.target.checked)}
            className="h-4 w-4 rounded border-border accent-brand cursor-pointer"
          />
          <span className="text-[13px] text-foreground">{field.name}</span>
        </label>
      );

    case "select":
      return <SelectEditor field={field} value={value} onChange={onChange} />;

    case "multi_select":
      return <MultiSelectEditor field={field} value={value} onChange={onChange} />;

    case "date":
      return (
        <Input
          type="date"
          value={dateToInputValue(value)}
          onChange={(e) => onChange(e.target.value ? new Date(e.target.value).toISOString() : null)}
          className="w-full"
        />
      );

    case "relation":
    case "rollup":
    case "formula":
    case "created_time":
    case "last_edited":
    case "files":
      return (
        <span className="text-muted-foreground text-[13px] italic">
          {field.fieldType === "created_time" || field.fieldType === "last_edited"
            ? "Auto-managed"
            : `${field.fieldType} (not editable inline)`}
        </span>
      );

    default:
      return (
        <Input
          type="text"
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.name}
          className="w-full"
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
      className="w-full rounded-lg border border-border bg-accent px-3 py-1.5 text-[13px] text-foreground outline-none transition-colors hover:border-border focus:border-brand/50 cursor-pointer"
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
    <div className="flex flex-wrap gap-1.5">
      {options.map((opt) => (
        <button
          key={opt}
          type="button"
          onClick={() => toggle(opt)}
          className={`rounded-md px-2.5 py-1 text-[12px] font-medium transition-colors cursor-pointer ${
            selected.includes(opt)
              ? "bg-brand/20 text-brand border border-brand/30"
              : "bg-accent text-muted-foreground border border-border hover:text-foreground"
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
