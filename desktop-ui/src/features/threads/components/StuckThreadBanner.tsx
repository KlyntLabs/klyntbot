type Props = {
  durationMs: number;
  onReset: () => void;
};

export function StuckThreadBanner({ durationMs, onReset }: Props): React.ReactElement {
  const seconds = Math.round(durationMs / 1000);
  return (
    <div className="stuck-thread-banner" role="alert">
      <span className="flex-1">
        This thread has been processing for {seconds}s with no response. It may be stuck.
      </span>
      <button type="button" className="px-2.5 py-1 bg-[var(--color-warning-fg)] text-[var(--color-bg)] border-none rounded text-ui-xs cursor-pointer" onClick={onReset}>
        Reset and try again
      </button>
    </div>
  );
}
