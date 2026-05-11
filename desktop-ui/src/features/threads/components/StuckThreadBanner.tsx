type Props = {
  durationMs: number;
  onReset: () => void;
};

export function StuckThreadBanner({ durationMs, onReset }: Props): React.ReactElement {
  const seconds = Math.round(durationMs / 1000);
  return (
    <div className="stuck-thread-banner" role="alert">
      <span className="stuck-thread-banner__msg">
        This thread has been processing for {seconds}s with no response. It may be stuck.
      </span>
      <button
        type="button"
        className="stuck-thread-banner__btn"
        onClick={onReset}
      >
        Reset and try again
      </button>
    </div>
  );
}
