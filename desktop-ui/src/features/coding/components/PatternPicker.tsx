import type { SuggestedGrant } from "./preview/types";

type Alternative = { pattern: string; label: string };

function deriveAlternatives(suggested: SuggestedGrant): Alternative[] {
  if (suggested.scope.kind === "tool_folder") {
    return [
      {
        pattern: `${suggested.scope.tool} on ${suggested.scope.folder}/**`,
        label: "deeper recursion",
      },
    ];
  }
  if (suggested.scope.kind === "exact_tool_path") {
    return [
      {
        pattern: `${suggested.scope.tool} in same folder`,
        label: "broaden to folder",
      },
    ];
  }
  return [];
}

type Props = {
  suggested: SuggestedGrant;
  onCommit: (rule: string) => void;
  onCustom: () => void;
};

export function PatternPicker({ suggested, onCommit, onCustom }: Props) {
  const alternatives = deriveAlternatives(suggested);
  return (
    <div className="approval-card__pattern-picker" role="radiogroup">
      <div>
        <button
          type="button"
          className="approval-card__pattern-picker-item approval-card__pattern-picker-item--suggested"
          onClick={() => onCommit(suggested.pattern)}
        >
          <strong>{suggested.pattern}</strong>
          <span className="approval-card__pattern-reason">{suggested.reason}</span>
        </button>
      </div>
      {alternatives.map((alt) => (
        <div key={alt.pattern}>
          <button
            type="button"
            className="approval-card__pattern-picker-item"
            onClick={() => onCommit(alt.pattern)}
          >
            {alt.pattern}
            <span className="approval-card__pattern-reason">{alt.label}</span>
          </button>
        </div>
      ))}
      <div>
        <button type="button" className="approval-card__pattern-picker-item" onClick={onCustom}>
          Custom Starlark rule...
        </button>
      </div>
    </div>
  );
}
