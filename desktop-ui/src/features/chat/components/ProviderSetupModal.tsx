import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import { SettingsModelsSection } from "@/features/settings/components/sections/SettingsModelsSection";
import X from "lucide-react/dist/esm/icons/x";

export type ProviderSetupModalProps = {
  onClose: () => void;
};

export function ProviderSetupModal({ onClose }: ProviderSetupModalProps) {
  return (
    <ModalShell
      className="settings-overlay"
      cardClassName="settings-window"
      onBackdropClick={onClose}
      ariaLabelledBy="provider-setup-modal-title"
    >
      <span className="settings-visually-hidden" id="provider-setup-modal-title">
        Provider setup
      </span>
      <button
        type="button"
        className="ghost icon-button settings-close"
        onClick={onClose}
        aria-label="Close provider setup"
      >
        <X aria-hidden />
      </button>
      <div className="settings-body">
        <div className="settings-content">
          <SettingsModelsSection />
        </div>
      </div>
    </ModalShell>
  );
}
