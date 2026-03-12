import type { LabelInterface } from "../mock-data/labels";

export function LabelBadge({ label }: { label: LabelInterface[] }) {
  return (
    <>
      {label.map((l) => (
        <span
          key={l.id}
          className="inline-flex items-center gap-1.5 rounded-full border border-[hsl(var(--border))] px-2.5 py-0.5 text-xs text-[hsl(var(--muted-foreground))] bg-[hsl(var(--background))]"
        >
          <span
            className="size-1.5 rounded-full"
            style={{ backgroundColor: l.color }}
            aria-hidden="true"
          />
          {l.name}
        </span>
      ))}
    </>
  );
}
