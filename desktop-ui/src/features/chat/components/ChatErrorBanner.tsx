type Props = {
  error: string | null;
  onDismiss: () => void;
};

export function ChatErrorBanner({ error, onDismiss }: Props) {
  if (error === null) return null;
  return (
    <div className="chat-error-banner" role="alert">
      <span className="chat-error-banner__message">{error}</span>
      <button
        type="button"
        className="chat-error-banner__dismiss"
        aria-label="Dismiss error"
        onClick={onDismiss}
      >
        ×
      </button>
    </div>
  );
}
