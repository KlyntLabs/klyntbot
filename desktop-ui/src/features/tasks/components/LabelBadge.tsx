import type { LabelInterface } from "../lib/mappers";

export function LabelBadge({ label }: { label: LabelInterface[] }) {
  return (
    <>
      {label.map((l) => (
        <span
          key={l.id}
          className="inline-flex items-center gap-1.5 rounded-full border border-separator px-2.5 py-0.5 text-ui-sm text-fg-secondary bg-bg"
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
