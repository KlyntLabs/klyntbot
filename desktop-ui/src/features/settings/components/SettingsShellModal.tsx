import X from "lucide-react/dist/esm/icons/x";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsShell } from "./SettingsShell";
import type { SettingsViewProps } from "./SettingsView";

export function SettingsShellModal({ onClose }: SettingsViewProps) {
  return (
    <ModalShell
      className="settings-overlay"
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
      <div className="settings-body">
        <SettingsShell />
      </div>
    </ModalShell>
  );
}
