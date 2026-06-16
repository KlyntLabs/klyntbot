import Settings from "lucide-react/dist/esm/icons/settings";

export type ProviderSetupPromptProps = {
  onOpenSettings: () => void;
};

export function ProviderSetupPrompt({ onOpenSettings }: ProviderSetupPromptProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center p-6 text-center">
      <div className="max-w-md rounded-2xl border border-[var(--border-subtle)] bg-[var(--surface-card)] p-8 shadow-sm">
        <h2 className="mb-2 text-[var(--fs-xl)] font-semibold text-[var(--text-strong)]">
          Set up a provider
        </h2>
        <p className="mb-6 text-[var(--fs-sm)] text-[var(--text-subtle)]">
          Add an API key for at least one provider, then choose a model to start chatting.
        </p>
        <button
          type="button"
          onClick={onOpenSettings}
          className="primary inline-flex items-center gap-2"
        >
          <Settings size={16} aria-hidden />
          <span>Open provider settings</span>
        </button>
      </div>
    </div>
  );
}
