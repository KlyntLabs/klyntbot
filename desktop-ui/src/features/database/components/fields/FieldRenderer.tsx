import type { FieldDefinition } from "@shared/types";

interface FieldRendererProps {
  field: FieldDefinition;
  value: unknown;
}

export function FieldRenderer({ field, value }: FieldRendererProps) {
  if (value == null) return <span className="text-muted">—</span>;

  switch (field.fieldType) {
    case "text":
    case "email":
    case "phone":
    case "person":
      return <span className="truncate">{String(value)}</span>;

    case "number":
      return <span className="tabular-nums">{Number(value).toLocaleString()}</span>;

    case "checkbox":
      return <span className={value ? "text-green-500" : "text-muted"}>{value ? "✓" : "✗"}</span>;

    case "select":
      return (
        <span className="inline-flex items-center rounded-full bg-surface-raised px-2 py-0.5 text-xs font-medium">
          {String(value)}
        </span>
      );

    case "multi_select": {
      const items = Array.isArray(value) ? value : [];
      return (
        <div className="flex flex-wrap gap-1">
          {items.map((item: string) => (
            <span
              key={item}
              className="inline-flex items-center rounded-full bg-surface-raised px-2 py-0.5 text-xs font-medium"
            >
              {item}
            </span>
          ))}
        </div>
      );
    }

    case "date":
    case "created_time":
    case "last_edited":
      return <span className="tabular-nums text-sm">{formatDate(String(value))}</span>;

    case "url":
      return (
        <a
          href={String(value)}
          target="_blank"
          rel="noopener noreferrer"
          className="text-accent hover:underline truncate"
        >
          {String(value)}
        </a>
      );

    case "relation": {
      const ids = Array.isArray(value) ? value : [];
      return <span className="text-muted text-sm">{ids.length} linked</span>;
    }

    case "rollup":
    case "formula":
      return <span className="text-muted italic">{String(value)}</span>;

    case "files": {
      const files = Array.isArray(value) ? value : [];
      return <span className="text-muted text-sm">{files.length} file(s)</span>;
    }

    default:
      return <span>{String(value)}</span>;
  }
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString();
  } catch {
    return iso;
  }
}
