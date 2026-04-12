import type { FieldType } from "@shared/types";

const FIELD_TYPES: { value: FieldType; label: string; icon: string }[] = [
  { value: "text", label: "Text", icon: "Aa" },
  { value: "number", label: "Number", icon: "#" },
  { value: "select", label: "Select", icon: "\u25BE" },
  { value: "multi_select", label: "Multi-Select", icon: "\u229E" },
  { value: "date", label: "Date", icon: "\uD83D\uDCC5" },
  { value: "checkbox", label: "Checkbox", icon: "\u2611" },
  { value: "url", label: "URL", icon: "\uD83D\uDD17" },
  { value: "email", label: "Email", icon: "\u2709" },
  { value: "phone", label: "Phone", icon: "\uD83D\uDCDE" },
  { value: "relation", label: "Relation", icon: "\u2194" },
  { value: "files", label: "Files", icon: "\uD83D\uDCCE" },
  { value: "person", label: "Person", icon: "\uD83D\uDC64" },
];

interface FieldTypeSelectorProps {
  value: FieldType;
  onChange: (value: FieldType) => void;
}

export function FieldTypeSelector({ value, onChange }: FieldTypeSelectorProps) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as FieldType)}
      className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
    >
      {FIELD_TYPES.map((ft) => (
        <option key={ft.value} value={ft.value}>
          {ft.icon} {ft.label}
        </option>
      ))}
    </select>
  );
}
