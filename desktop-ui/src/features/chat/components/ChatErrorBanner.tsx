type Props = {
  error: string | null;
  onDismiss: () => void;
};

export function ChatErrorBanner({ error, onDismiss }: Props) {
  if (error === null) return null;
  return (
    <div className="chat-error-banner" role="alert">
      <span className="flex-1 whitespace-pre-wrap">{error}</span>
      <button
        type="button"
        className="bg-transparent border-none cursor-pointer text-inherit text-[length:var(--fs-md)] leading-none px-0.5 py-1.5 hover:opacity-70"
        aria-label="Dismiss error"
        onClick={onDismiss}
      >
        ×
      </button>
    </div>
  );
}
