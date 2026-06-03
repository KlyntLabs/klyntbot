import X from "lucide-react/dist/esm/icons/x";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsShell } from "./SettingsShell";
import type { SettingsViewProps } from "./SettingsView";

export function SettingsShellModal({ onClose }: SettingsViewProps) {
  return (
    <ModalShell
      className="fixed inset-0 z-ui-modal flex items-center justify-center p-6 bg-black/80"
      cardClassName="settings-window"
      onBackdropClick={onClose}
      ariaLabelledBy="new-settings-modal-title"
    >
      <span className="settings-visually-hidden" id="new-settings-modal-title">
        Settings
      </span>
      <button
        type="button"
        className="ghost icon-button settings-close"
        onClick={onClose}
        aria-label="Close settings"
      >
        <X aria-hidden />
      </button>
      <div className="w-[min(520px,calc(100vw-48px))] max-h-[min(640px,calc(100vh-120px))] overflow-auto rounded-2xl bg-surface-card border border-border-subtle shadow-[0_24px_60px_rgba(0,0,0,0.5)] p-6">
        <SettingsShell />
      </div>
    </ModalShell>
  );
}
