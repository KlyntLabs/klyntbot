interface Props {
  kinds: string[];
  value: string;
  onChange: (kind: string) => void;
}

export function WireFilters({ kinds, value, onChange }: Props) {
  return (
    <div className="tracing-filters">
      <span className="tracing-filters__label">Filter:</span>
      <button
        type="button"
        className={"tracing-filters__chip" + (!value ? " tracing-filters__chip--active" : "")}
        onClick={() => onChange("")}
      >
        All
      </button>
      {kinds.map((k) => (
        <button
          key={k}
          type="button"
          className={
            "tracing-filters__chip" + (value === k ? " tracing-filters__chip--active" : "")
          }
          onClick={() => onChange(k)}
        >
          {k}
        </button>
      ))}
    </div>
  );
}
