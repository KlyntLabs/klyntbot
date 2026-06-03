import {
  SettingsField,
  SettingsFieldLabel,
  SettingsHelpText,
  SettingsSection,
  SettingsSelect,
  SettingsSubsection,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { AppSettings } from "@/types";
import { cn } from "@/utils/cn";

type ComposerPreset = AppSettings["composerEditorPreset"];

type SettingsComposerSectionProps = {
  appSettings: AppSettings;
  optionKeyLabel: string;
  followUpShortcutLabel: string;
  composerPresetLabels: Record<ComposerPreset, string>;
  onComposerPresetChange: (preset: ComposerPreset) => void;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
};

export function SettingsComposerSection({
  appSettings,
  optionKeyLabel,
  followUpShortcutLabel,
  composerPresetLabels,
  onComposerPresetChange,
  onUpdateAppSettings,
}: SettingsComposerSectionProps) {
  const steerUnavailable = !appSettings.steerEnabled;
  return (
    <SettingsSection
      title="Composer"
      subtitle="Control helpers and formatting behavior inside the message editor."
    >
      <SettingsField>
        <SettingsFieldLabel>Follow-up behavior</SettingsFieldLabel>
        <div
          className="relative inline-flex items-center self-start gap-1 p-1 rounded-full border border-border-muted bg-surface-control"
          role="radiogroup"
          aria-label="Follow-up behavior"
        >
          <div
            className={cn(
              "absolute top-1 left-1 h-[calc(100%-8px)] rounded-full bg-surface-card shadow-[inset_0_0_0_1px_var(--border-strong)] z-0 transition-transform duration-200 w-[calc(50%-6px)]",
              appSettings.followUpMessageBehavior === "steer"
                ? "translate-x-[calc(100%+4px)]"
                : "translate-x-0",
            )}
          />
          <label
            className={cn(
              "relative z-[1] inline-flex items-center justify-center rounded-full bg-transparent text-ui-sm font-semibold p-0 min-w-[72px] overflow-hidden transition-colors duration-200",
              appSettings.followUpMessageBehavior === "queue"
                ? "text-text-strong"
                : "text-text-muted hover:text-text-strong hover:bg-surface-card/55",
            )}
          >
            <input
              className="absolute inset-0 opacity-0 m-0"
              type="radio"
              name="follow-up-behavior"
              value="queue"
              checked={appSettings.followUpMessageBehavior === "queue"}
              onChange={() =>
                void onUpdateAppSettings({
                  ...appSettings,
                  followUpMessageBehavior: "queue",
                })
              }
            />
            <span className="inline-flex items-center justify-center w-full px-3 py-1.5">
              Queue
            </span>
          </label>
          <label
            className={cn(
              "relative z-[1] inline-flex items-center justify-center rounded-full bg-transparent text-ui-sm font-semibold p-0 min-w-[72px] overflow-hidden transition-colors duration-200",
              appSettings.followUpMessageBehavior === "steer"
                ? "text-text-strong"
                : "text-text-muted",
              steerUnavailable
                ? "cursor-not-allowed text-text-faint"
                : "hover:text-text-strong hover:bg-surface-card/55",
            )}
            title={steerUnavailable ? "Steer is unavailable in the current Codex config." : ""}
          >
            <input
              className="absolute inset-0 opacity-0 m-0"
              type="radio"
              name="follow-up-behavior"
              value="steer"
              checked={appSettings.followUpMessageBehavior === "steer"}
              disabled={steerUnavailable}
              onChange={() => {
                if (steerUnavailable) {
                  return;
                }
                void onUpdateAppSettings({
                  ...appSettings,
                  followUpMessageBehavior: "steer",
                });
              }}
            />
            <span className="inline-flex items-center justify-center w-full px-3 py-1.5">
              Steer
            </span>
          </label>
        </div>
        <SettingsHelpText>
          Choose the default while a run is active. Press {followUpShortcutLabel} to send the
          opposite behavior for one message.
        </SettingsHelpText>
        <SettingsToggleRow
          title="Show follow-up hint while processing"
          subtitle="Displays queue/steer shortcut guidance above the composer."
        >
          <SettingsToggleSwitch
            pressed={appSettings.composerFollowUpHintEnabled}
            onClick={() =>
              void onUpdateAppSettings({
                ...appSettings,
                composerFollowUpHintEnabled: !appSettings.composerFollowUpHintEnabled,
              })
            }
          />
        </SettingsToggleRow>
        {steerUnavailable && (
          <SettingsHelpText>
            Steer is unavailable in the current Codex config. Follow-ups will queue.
          </SettingsHelpText>
        )}
      </SettingsField>
      <div className="h-px bg-border-muted my-4 rounded-full" />
      <SettingsSubsection
        title="Presets"
        subtitle="Choose a starting point and fine-tune the toggles below."
      />
      <SettingsField>
        <SettingsFieldLabel htmlFor="composer-preset">Preset</SettingsFieldLabel>
        <SettingsSelect
          id="composer-preset"
          value={appSettings.composerEditorPreset}
          onChange={(event) => onComposerPresetChange(event.target.value as ComposerPreset)}
        >
          {Object.entries(composerPresetLabels).map(([preset, label]) => (
            <option key={preset} value={preset}>
              {label}
            </option>
          ))}
        </SettingsSelect>
        <SettingsHelpText>
          Presets update the toggles below. Customize any setting after selecting.
        </SettingsHelpText>
      </SettingsField>
      <div className="h-px bg-border-muted my-4 rounded-full" />
      <SettingsSubsection title="Code fences" />
      <SettingsToggleRow
        title="Expand fences on Space"
        subtitle="Typing ``` then Space inserts a fenced block."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceExpandOnSpace}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceExpandOnSpace: !appSettings.composerFenceExpandOnSpace,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Expand fences on Enter"
        subtitle="Use Enter to expand ``` lines when enabled."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceExpandOnEnter}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceExpandOnEnter: !appSettings.composerFenceExpandOnEnter,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Support language tags"
        subtitle="Allows ```lang + Space to include a language."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceLanguageTags}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceLanguageTags: !appSettings.composerFenceLanguageTags,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Wrap selection in fences"
        subtitle="Wraps selected text when creating a fence."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceWrapSelection}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceWrapSelection: !appSettings.composerFenceWrapSelection,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Copy blocks without fences"
        subtitle={
          <>When enabled, Copy is plain text. Hold {optionKeyLabel} to include ``` fences.</>
        }
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerCodeBlockCopyUseModifier}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerCodeBlockCopyUseModifier: !appSettings.composerCodeBlockCopyUseModifier,
            })
          }
        />
      </SettingsToggleRow>
      <div className="h-px bg-border-muted my-4 rounded-full" />
      <SettingsSubsection title="Pasting" />
      <SettingsToggleRow
        title="Auto-wrap multi-line paste"
        subtitle="Wraps multi-line paste inside a fenced block."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceAutoWrapPasteMultiline}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceAutoWrapPasteMultiline: !appSettings.composerFenceAutoWrapPasteMultiline,
            })
          }
        />
      </SettingsToggleRow>
      <SettingsToggleRow
        title="Auto-wrap code-like single lines"
        subtitle="Wraps long single-line code snippets on paste."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerFenceAutoWrapPasteCodeLike}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerFenceAutoWrapPasteCodeLike: !appSettings.composerFenceAutoWrapPasteCodeLike,
            })
          }
        />
      </SettingsToggleRow>
      <div className="h-px bg-border-muted my-4 rounded-full" />
      <SettingsSubsection title="Lists" />
      <SettingsToggleRow
        title="Continue lists on Shift+Enter"
        subtitle="Continues numbered and bulleted lists when the line has content."
      >
        <SettingsToggleSwitch
          pressed={appSettings.composerListContinuation}
          onClick={() =>
            void onUpdateAppSettings({
              ...appSettings,
              composerListContinuation: !appSettings.composerListContinuation,
            })
          }
        />
      </SettingsToggleRow>
    </SettingsSection>
  );
}
