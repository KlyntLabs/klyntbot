import { ipc } from "@shared/hooks/useIpc";
import { useState } from "react";

type Step = "install" | "confirm";

interface Props {
  open: boolean;
  onClose: () => void;
  onInstalled: () => void;
}

export function FocusOnboarding({ open, onClose, onInstalled }: Props) {
  const [step, setStep] = useState<Step>("install");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (!open) return null;

  const handleInstall = () => {
    setBusy(true);
    setError(null);
    ipc("focus_install_shortcuts")
      .then(() => setStep("confirm"))
      .catch((err) => setError(String(err)))
      .finally(() => setBusy(false));
  };

  const handleDone = () => {
    setBusy(true);
    setError(null);
    ipc<boolean>("focus_shortcuts_installed")
      .then((installed) => {
        if (installed) {
          onInstalled();
          onClose();
        } else {
          setError("Didn't detect the shortcuts — try tapping Add Shortcut in both dialogs.");
        }
      })
      .catch((err) => setError(String(err)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents lint/a11y/noStaticElementInteractions: modal backdrop - dismiss via click or Escape key */}
      <div className="absolute inset-0 bg-black/40" onClick={onClose} />

      <div className="relative z-10 glass-panel rounded-xl p-6 max-w-sm w-full mx-4 shadow-xl">
        {step === "install" ? (
          <>
            <h2 className="text-sm font-semibold text-fg mb-2">Set up DND timers</h2>
            <p className="text-ui-xs text-fg-secondary mb-4">
              To use DND timers, klyntbot needs to install two macOS Shortcuts — one to turn DND on
              and one to turn it off.
            </p>
            {error && <p className="text-ui-sm text-status-danger mb-3">{error}</p>}
            <div className="flex gap-2 justify-end">
              <button
                type="button"
                className="px-3 py-1.5 text-ui-xs text-fg-secondary hover:text-fg transition-colors"
                onClick={onClose}
                disabled={busy}
              >
                Cancel
              </button>
              <button
                type="button"
                className="px-3 py-1.5 text-ui-sm rounded bg-control-hover text-fg hover:bg-control-hover/80 transition-colors disabled:opacity-50"
                onClick={handleInstall}
                disabled={busy}
              >
                {busy ? "Opening…" : "Install"}
              </button>
            </div>
          </>
        ) : (
          <>
            <h2 className="text-sm font-semibold text-fg mb-2">Add both shortcuts</h2>
            <p className="text-ui-xs text-fg-secondary mb-4">
              Shortcuts.app has opened two files. Tap{" "}
              <strong className="text-fg">Add Shortcut</strong> in each dialog, then click Done.
            </p>
            {error && <p className="text-ui-sm text-status-danger mb-3">{error}</p>}
            <div className="flex gap-2 justify-end">
              <button
                type="button"
                className="px-3 py-1.5 text-ui-xs text-fg-secondary hover:text-fg transition-colors"
                onClick={onClose}
                disabled={busy}
              >
                Cancel
              </button>
              <button
                type="button"
                className="px-3 py-1.5 text-ui-sm rounded bg-control-hover text-fg hover:bg-control-hover/80 transition-colors disabled:opacity-50"
                onClick={handleDone}
                disabled={busy}
              >
                {busy ? "Checking…" : "Done"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

/** Expose open/element so callers don't need local state boilerplate. */
export function useFocusOnboarding() {
  const [open, setOpen] = useState(false);
  const [onInstalledCb, setOnInstalledCb] = useState<(() => void) | null>(null);

  const openOnboarding = (onInstalled: () => void) => {
    setOnInstalledCb(() => onInstalled);
    setOpen(true);
  };

  const element = (
    <FocusOnboarding
      open={open}
      onClose={() => setOpen(false)}
      onInstalled={() => {
        setOpen(false);
        onInstalledCb?.();
      }}
    />
  );

  return { open: openOnboarding, element };
}
