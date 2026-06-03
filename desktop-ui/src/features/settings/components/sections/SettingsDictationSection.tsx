import { formatDownloadSize } from "@utils/formatting";
import {
  SettingsField,
  SettingsFieldLabel,
  SettingsHelpText,
  SettingsSection,
  SettingsSelect,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { AppSettings, DictationModelStatus } from "@/types";

type DictationModelOption = {
  id: string;
  label: string;
  size: string;
  note: string;
};

type SettingsDictationSectionProps = {
  appSettings: AppSettings;
  optionKeyLabel: string;
  metaKeyLabel: string;
  dictationModels: DictationModelOption[];
  selectedDictationModel: DictationModelOption;
  dictationModelStatus?: DictationModelStatus | null;
  dictationReady: boolean;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  onDownloadDictationModel?: () => void;
  onCancelDictationDownload?: () => void;
  onRemoveDictationModel?: () => void;
};

export function SettingsDictationSection({
  appSettings,
  optionKeyLabel,
  metaKeyLabel,
  dictationModels,
  selectedDictationModel,
  dictationModelStatus,
  dictationReady,
  onUpdateAppSettings,
  onDownloadDictationModel,
  onCancelDictationDownload,
  onRemoveDictationModel,
}: SettingsDictationSectionProps) {
  const dictationProgress = dictationModelStatus?.progress ?? null;

  return (
    <SettingsSection
      title="Dictation"
      subtitle="Enable microphone dictation with on-device transcription."
    >
      <SettingsToggleRow
        title="Enable dictation"
        subtitle="Downloads the selected Whisper model on first use."
      >
        <SettingsToggleSwitch
          pressed={appSettings.dictationEnabled}
          onClick={() => {
            const nextEnabled = !appSettings.dictationEnabled;
            void onUpdateAppSettings({
              ...appSettings,
              dictationEnabled: nextEnabled,
            });
            if (
              !nextEnabled &&
              dictationModelStatus?.state === "downloading" &&
              onCancelDictationDownload
            ) {
              onCancelDictationDownload();
            }
            if (
              nextEnabled &&
              dictationModelStatus?.state === "missing" &&
              onDownloadDictationModel
            ) {
              onDownloadDictationModel();
            }
          }}
        />
      </SettingsToggleRow>
      <SettingsField>
        <SettingsFieldLabel htmlFor="dictation-model">Dictation model</SettingsFieldLabel>
        <SettingsSelect
          id="dictation-model"
          value={appSettings.dictationModelId}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              dictationModelId: event.target.value,
            })
          }
        >
          {dictationModels.map((model) => (
            <option key={model.id} value={model.id}>
              {model.label} ({model.size})
            </option>
          ))}
        </SettingsSelect>
        <SettingsHelpText>
          {selectedDictationModel.note} Download size: {selectedDictationModel.size}.
        </SettingsHelpText>
      </SettingsField>
      <SettingsField>
        <SettingsFieldLabel htmlFor="dictation-language">
          Preferred dictation language
        </SettingsFieldLabel>
        <SettingsSelect
          id="dictation-language"
          value={appSettings.dictationPreferredLanguage ?? ""}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              dictationPreferredLanguage: event.target.value || null,
            })
          }
        >
          <option value="">Auto-detect only</option>
          <option value="en">English</option>
          <option value="es">Spanish</option>
          <option value="fr">French</option>
          <option value="de">German</option>
          <option value="it">Italian</option>
          <option value="pt">Portuguese</option>
          <option value="nl">Dutch</option>
          <option value="sv">Swedish</option>
          <option value="no">Norwegian</option>
          <option value="da">Danish</option>
          <option value="fi">Finnish</option>
          <option value="pl">Polish</option>
          <option value="tr">Turkish</option>
          <option value="ru">Russian</option>
          <option value="uk">Ukrainian</option>
          <option value="ja">Japanese</option>
          <option value="ko">Korean</option>
          <option value="zh">Chinese</option>
        </SettingsSelect>
        <SettingsHelpText>
          Auto-detect stays on; this nudges the decoder toward your preference.
        </SettingsHelpText>
      </SettingsField>
      <SettingsField>
        <SettingsFieldLabel htmlFor="dictation-hold-key">Hold-to-dictate key</SettingsFieldLabel>
        <SettingsSelect
          id="dictation-hold-key"
          value={appSettings.dictationHoldKey ?? ""}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              dictationHoldKey: event.target.value,
            })
          }
        >
          <option value="">Off</option>
          <option value="alt">{optionKeyLabel}</option>
          <option value="shift">Shift</option>
          <option value="control">Control</option>
          <option value="meta">{metaKeyLabel}</option>
        </SettingsSelect>
        <SettingsHelpText>
          Hold the key to start dictation, release to stop and process.
        </SettingsHelpText>
      </SettingsField>
      {dictationModelStatus && (
        <SettingsField>
          <SettingsFieldLabel>
            Model status ({selectedDictationModel.label})
          </SettingsFieldLabel>
          <SettingsHelpText>
            {dictationModelStatus.state === "ready" && "Ready for dictation."}
            {dictationModelStatus.state === "missing" && "Model not downloaded yet."}
            {dictationModelStatus.state === "downloading" && "Downloading model..."}
            {dictationModelStatus.state === "error" &&
              (dictationModelStatus.error ?? "Download error.")}
          </SettingsHelpText>
          {dictationProgress && (
            <div className="flex flex-col gap-1.5">
              <div className="h-1.5 rounded-full bg-surface-control border border-border-muted overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-[rgba(100,200,255,0.7)] to-[rgba(120,235,190,0.8)]"
                  style={{
                    width: dictationProgress.totalBytes
                      ? `${Math.min(
                          100,
                          (dictationProgress.downloadedBytes / dictationProgress.totalBytes) * 100,
                        )}%`
                      : "0%",
                  }}
                />
              </div>
              <div className="text-ui-xs text-text-muted">
                {formatDownloadSize(dictationProgress.downloadedBytes)}
              </div>
            </div>
          )}
          <div className="flex gap-2.5 items-center">
            {dictationModelStatus.state === "missing" && (
              <button
                type="button"
                className="primary"
                onClick={onDownloadDictationModel}
                disabled={!onDownloadDictationModel}
              >
                Download model
              </button>
            )}
            {dictationModelStatus.state === "downloading" && (
              <button
                type="button"
                className="ghost py-1.5 px-2.5 text-ui-sm"
                onClick={onCancelDictationDownload}
                disabled={!onCancelDictationDownload}
              >
                Cancel download
              </button>
            )}
            {dictationReady && (
              <button
                type="button"
                className="ghost py-1.5 px-2.5 text-ui-sm"
                onClick={onRemoveDictationModel}
                disabled={!onRemoveDictationModel}
              >
                Remove model
              </button>
            )}
          </div>
        </SettingsField>
      )}
    </SettingsSection>
  );
}
