import {
  SettingsField,
  SettingsFieldLabel,
  SettingsHelpText,
  SettingsSection,
  SettingsSelect,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { AppSettings, ModelOption } from "@/types";

type SettingsGitSectionProps = {
  appSettings: AppSettings;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  models: ModelOption[];
  commitMessagePromptDraft: string;
  commitMessagePromptDirty: boolean;
  commitMessagePromptSaving: boolean;
  onSetCommitMessagePromptDraft: (value: string) => void;
  onSaveCommitMessagePrompt: () => Promise<void>;
  onResetCommitMessagePrompt: () => Promise<void>;
};

export function SettingsGitSection({
  appSettings,
  onUpdateAppSettings,
  models,
  commitMessagePromptDraft,
  commitMessagePromptDirty,
  commitMessagePromptSaving,
  onSetCommitMessagePromptDraft,
  onSaveCommitMessagePrompt,
  onResetCommitMessagePrompt,
}: SettingsGitSectionProps) {
  return (
    <SettingsSection title="Git" subtitle="Manage how diffs are loaded in the Git sidebar.">
      <SettingsToggleRow title="Preload git diffs" subtitle="Make viewing git diff faster.">
        <SettingsToggleSwitch
          pressed={appSettings.preloadGitDiffs}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              preloadGitDiffs: !appSettings.preloadGitDiffs,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Ignore whitespace changes"
        subtitle="Hides whitespace-only changes in local and commit diffs."
      >
        <SettingsToggleSwitch
          pressed={appSettings.gitDiffIgnoreWhitespaceChanges}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              gitDiffIgnoreWhitespaceChanges: !appSettings.gitDiffIgnoreWhitespaceChanges,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsField>
        <SettingsFieldLabel>Commit message prompt</SettingsFieldLabel>
        <SettingsHelpText>
          Used when generating commit messages. Include <code>{"{diff}"}</code> to insert the git
          diff.
        </SettingsHelpText>
        <textarea
          className="w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
          value={commitMessagePromptDraft}
          onChange={(event) => onSetCommitMessagePromptDraft(event.target.value)}
          spellCheck={false}
          disabled={commitMessagePromptSaving}
        />
        <div className="flex gap-2.5 items-center">
          <button
            type="button"
            className="ghost py-1.5 px-2.5 text-ui-sm"
            onClick={() => {
              void onResetCommitMessagePrompt();
            }}
            disabled={commitMessagePromptSaving || !commitMessagePromptDirty}
          >
            Reset
          </button>
          <button
            type="button"
            className="primary py-1.5 px-2.5 text-ui-sm"
            onClick={() => {
              void onSaveCommitMessagePrompt();
            }}
            disabled={commitMessagePromptSaving || !commitMessagePromptDirty}
          >
            {commitMessagePromptSaving ? "Saving..." : "Save"}
          </button>
        </div>
      </SettingsField>
      {models.length > 0 && (
        <SettingsField>
          <SettingsFieldLabel htmlFor="commit-message-model-select">
            Commit message model
          </SettingsFieldLabel>
          <SettingsHelpText>
            The model used when generating commit messages. Leave on default to use the workspace
            model.
          </SettingsHelpText>
          <SettingsSelect
            id="commit-message-model-select"
            value={appSettings.commitMessageModelId ?? ""}
            onChange={(event) => {
              const value = event.target.value || null;
              void onUpdateAppSettings({
                ...appSettings,
                commitMessageModelId: value,
              });
            }}
          >
            <option value="">Default</option>
            {models.map((model) => (
              <option key={model.id} value={model.model}>
                {model.displayName?.trim() || model.model}
              </option>
            ))}
          </SettingsSelect>
        </SettingsField>
      )}
    </SettingsSection>
  );
}
