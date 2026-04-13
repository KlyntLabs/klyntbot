import type { FieldDefinition } from "@shared/types";

interface FieldRendererProps {
  field: FieldDefinition;
  value: unknown;
}

/** Notion-style muted color palette for select badges — deterministic by value string */
const BADGE_COLORS = [
  { bg: "bg-[oklch(0.35_0.05_250)]", text: "text-[oklch(0.75_0.1_250)]" }, // blue
  { bg: "bg-[oklch(0.35_0.05_165)]", text: "text-[oklch(0.75_0.12_165)]" }, // teal
  { bg: "bg-[oklch(0.35_0.05_293)]", text: "text-[oklch(0.75_0.1_293)]" }, // purple
  { bg: "bg-[oklch(0.35_0.06_25)]", text: "text-[oklch(0.75_0.12_25)]" }, // orange
  { bg: "bg-[oklch(0.35_0.05_84)]", text: "text-[oklch(0.78_0.12_84)]" }, // yellow
  { bg: "bg-[oklch(0.35_0.06_340)]", text: "text-[oklch(0.75_0.1_340)]" }, // pink
  { bg: "bg-[oklch(0.35_0.05_145)]", text: "text-[oklch(0.75_0.12_145)]" }, // green
  { bg: "bg-[oklch(0.3_0.02_0)]", text: "text-[oklch(0.7_0.01_0)]" }, // gray
] as const;

function hashStringToIndex(str: string, len: number): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) | 0;
  }
  return Math.abs(hash) % len;
}

function SelectBadge({ value }: { value: string }) {
  const color = BADGE_COLORS[hashStringToIndex(value, BADGE_COLORS.length)];
  return (
    <span
      className={`inline-flex items-center rounded-[4px] px-[6px] py-[1px] text-[12px] font-medium ${color.bg} ${color.text}`}
    >
      {value}
    </span>
  );
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
      return (
        <span className={value ? "text-success" : "text-dim"}>
          {value ? (
            <svg
              className="inline h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              strokeWidth={2}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
            </svg>
          ) : (
            <svg
              className="inline h-4 w-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M5.25 7.5A2.25 2.25 0 0 1 7.5 5.25h9a2.25 2.25 0 0 1 2.25 2.25v9a2.25 2.25 0 0 1-2.25 2.25h-9a2.25 2.25 0 0 1-2.25-2.25v-9Z"
              />
            </svg>
          )}
        </span>
      );

    case "select":
      return <SelectBadge value={String(value)} />;

    case "multi_select": {
      const items = Array.isArray(value) ? value : [];
      return (
        <div className="flex flex-wrap gap-1">
          {items.map((item: string) => (
            <SelectBadge key={item} value={item} />
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
          onClick={(e) => e.stopPropagation()}
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
    return new Date(iso).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  } catch {
    return iso;
  }
}
