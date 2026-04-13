import type { FieldDefinition } from "@shared/types";
import { Badge } from "@shared/ui/Badge";

interface FieldRendererProps {
  field: FieldDefinition;
  value: unknown;
}

export function FieldRenderer({ field, value }: FieldRendererProps) {
  if (value == null) return <span className="text-dim">—</span>;

  switch (field.fieldType) {
    case "text":
    case "email":
    case "phone":
    case "person":
      return <span className="truncate text-foreground">{String(value)}</span>;

    case "number":
      return <span className="tabular-nums text-foreground">{Number(value).toLocaleString()}</span>;

    case "checkbox":
      return <span className={value ? "text-success" : "text-dim"}>{value ? "✓" : "✗"}</span>;

    case "select":
      return <Badge size="sm">{String(value)}</Badge>;

    case "multi_select": {
      const items = Array.isArray(value) ? value : [];
      return (
        <div className="flex flex-wrap gap-1">
          {items.map((item: string) => (
            <Badge key={item} size="sm">
              {item}
            </Badge>
          ))}
        </div>
      );
    }

    case "date":
    case "created_time":
    case "last_edited":
      return (
        <span className="tabular-nums text-muted-foreground">{formatDate(String(value))}</span>
      );

    case "url":
      return (
        <a
          href={String(value)}
          target="_blank"
          rel="noopener noreferrer"
          className="text-brand hover:underline truncate"
        >
          {String(value)}
        </a>
      );

    case "relation": {
      const ids = Array.isArray(value) ? value : [];
      return <span className="text-muted-foreground">{ids.length} linked</span>;
    }

    case "rollup":
    case "formula":
      return <span className="text-muted-foreground italic">{String(value)}</span>;

    case "files": {
      const files = Array.isArray(value) ? value : [];
      return <span className="text-muted-foreground">{files.length} file(s)</span>;
    }

    default:
      return <span className="text-foreground">{String(value)}</span>;
  }
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleDateString();
  } catch {
    return iso;
  }
}
