interface ConfidenceTapProps {
  value: number;
  onChange: (rating: number) => void;
}

export function ConfidenceTap({ value, onChange }: ConfidenceTapProps) {
  return (
    <span className="inline-flex gap-0.5 text-sm select-none">
      {[1, 2, 3, 4, 5].map((star) => (
        <button
          key={star}
          type="button"
          onClick={() => onChange(star)}
          className={`cursor-pointer transition-colors ${
            star <= value ? "text-yellow-400" : "text-muted"
          }`}
        >
          {star <= value ? "\u2605" : "\u2606"}
        </button>
      ))}
    </span>
  );
}
