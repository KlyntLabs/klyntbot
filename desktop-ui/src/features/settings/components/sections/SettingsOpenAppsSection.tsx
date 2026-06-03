import { GENERIC_APP_ICON, getKnownOpenAppIcon } from "@app/utils/openAppIcons";
import type { OpenAppDraft } from "@settings/components/settingsTypes";
import { fileManagerName, isMacPlatform } from "@utils/platformPaths";
import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronUp from "lucide-react/dist/esm/icons/chevron-up";
import Trash2 from "lucide-react/dist/esm/icons/trash-2";
import {
  SettingsHelpText,
  SettingsInput,
  SettingsSection,
  SettingsSelect,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { OpenAppTarget } from "@/types";
import { cn } from "@/utils/cn";

type SettingsOpenAppsSectionProps = {
  openAppDrafts: OpenAppDraft[];
  openAppSelectedId: string;
  openAppIconById: Record<string, string>;
  onOpenAppDraftChange: (index: number, updates: Partial<OpenAppDraft>) => void;
  onOpenAppKindChange: (index: number, kind: OpenAppTarget["kind"]) => void;
  onCommitOpenApps: () => void;
  onMoveOpenApp: (index: number, direction: "up" | "down") => void;
  onDeleteOpenApp: (index: number) => void;
  onAddOpenApp: () => void;
  onSelectOpenAppDefault: (id: string) => void;
};

const isOpenAppLabelValid = (label: string) => label.trim().length > 0;

export function SettingsOpenAppsSection({
  openAppDrafts,
  openAppSelectedId,
  openAppIconById,
  onOpenAppDraftChange,
  onOpenAppKindChange,
  onCommitOpenApps,
  onMoveOpenApp,
  onDeleteOpenApp,
  onAddOpenApp,
  onSelectOpenAppDefault,
}: SettingsOpenAppsSectionProps) {
  return (
    <SettingsSection
      title="Open in"
      subtitle="Customize the Open in menu shown in the title bar and file previews."
    >
      <div className="flex flex-col gap-2">
        {openAppDrafts.map((target, index) => {
          const iconSrc =
            getKnownOpenAppIcon(target.id) ?? openAppIconById[target.id] ?? GENERIC_APP_ICON;
          const labelValid = isOpenAppLabelValid(target.label);
          const appNameValid = target.kind !== "app" || Boolean(target.appName?.trim());
          const commandValid = target.kind !== "command" || Boolean(target.command?.trim());
          const isComplete = labelValid && appNameValid && commandValid;
          const incompleteHint = !labelValid
            ? "Label required"
            : target.kind === "app"
              ? "App name required"
              : target.kind === "command"
                ? "Command required"
                : "Complete required fields";

          return (
            <div
              key={target.id}
              className={cn(
                "flex items-center gap-2.5 p-2 px-2.5 rounded-xl border bg-surface-card flex-wrap",
                isComplete ? "border-border-muted" : "border-status-error/50",
              )}
            >
              <div
                className="shrink-0 w-6 h-6 rounded-lg border border-border-muted bg-surface-control inline-flex items-center justify-center overflow-hidden"
                aria-hidden
              >
                <img
                  className="w-[18px] h-[18px] rounded-[5px]"
                  src={iconSrc}
                  alt=""
                  width={18}
                  height={18}
                />
              </div>
              <div className="flex-1 min-w-0 flex items-center gap-2 flex-wrap">
                <label className="min-w-0 inline-flex items-center">
                  <span className="sr-only">Label</span>
                  <SettingsInput
                    compact
                    className="w-[140px]"
                    value={target.label}
                    placeholder="Label"
                    onChange={(event) =>
                      onOpenAppDraftChange(index, {
                        label: event.target.value,
                      })
                    }
                    onBlur={onCommitOpenApps}
                    aria-label={`Open app label ${index + 1}`}
                    data-invalid={!labelValid || undefined}
                  />
                </label>
                <label className="min-w-0 inline-flex items-center">
                  <span className="sr-only">Type</span>
                  <SettingsSelect
                    className="w-24 min-w-24 py-1.5 px-2 text-ui-xs"
                    value={target.kind}
                    onChange={(event) =>
                      onOpenAppKindChange(index, event.target.value as OpenAppTarget["kind"])
                    }
                    aria-label={`Open app type ${index + 1}`}
                  >
                    <option value="app">App</option>
                    <option value="command">Command</option>
                    <option value="finder">{fileManagerName()}</option>
                  </SettingsSelect>
                </label>
                {target.kind === "app" && (
                  <label className="min-w-0 inline-flex items-center">
                    <span className="sr-only">App name</span>
                    <SettingsInput
                      compact
                      className="w-[220px] max-w-[240px]"
                      value={target.appName ?? ""}
                      placeholder="App name"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          appName: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app name ${index + 1}`}
                      data-invalid={!appNameValid || undefined}
                    />
                  </label>
                )}
                {target.kind === "command" && (
                  <label className="min-w-0 inline-flex items-center">
                    <span className="sr-only">Command</span>
                    <SettingsInput
                      compact
                      className="w-[200px] max-w-[220px]"
                      value={target.command ?? ""}
                      placeholder="Command"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          command: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app command ${index + 1}`}
                      data-invalid={!commandValid || undefined}
                    />
                  </label>
                )}
                {target.kind !== "finder" && (
                  <label className="min-w-0 inline-flex items-center">
                    <span className="sr-only">Args</span>
                    <SettingsInput
                      compact
                      className="flex-1 min-w-[140px]"
                      value={target.argsText}
                      placeholder="Args"
                      onChange={(event) =>
                        onOpenAppDraftChange(index, {
                          argsText: event.target.value,
                        })
                      }
                      onBlur={onCommitOpenApps}
                      aria-label={`Open app args ${index + 1}`}
                    />
                  </label>
                )}
              </div>
              <div className="inline-flex items-center gap-1.5 ml-auto shrink-0">
                {!isComplete && (
                  <span
                    className="text-ui-xs text-status-error px-1.5 py-0.5 rounded-full border border-status-error/40"
                    role="status"
                    title={incompleteHint}
                    aria-label={incompleteHint}
                  >
                    Incomplete
                  </span>
                )}
                <label className="inline-flex items-center gap-1 text-ui-xs text-text-muted">
                  <input
                    type="radio"
                    name="open-app-default"
                    checked={target.id === openAppSelectedId}
                    onChange={() => onSelectOpenAppDefault(target.id)}
                    disabled={!isComplete}
                  />
                  Default
                </label>
                <div className="inline-flex gap-1">
                  <button
                    type="button"
                    className="ghost icon-button"
                    onClick={() => onMoveOpenApp(index, "up")}
                    disabled={index === 0}
                    aria-label="Move up"
                  >
                    <ChevronUp aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="ghost icon-button"
                    onClick={() => onMoveOpenApp(index, "down")}
                    disabled={index === openAppDrafts.length - 1}
                    aria-label="Move down"
                  >
                    <ChevronDown aria-hidden />
                  </button>
                </div>
                <button
                  type="button"
                  className="ghost icon-button"
                  onClick={() => onDeleteOpenApp(index)}
                  disabled={openAppDrafts.length <= 1}
                  aria-label="Remove app"
                  title="Remove app"
                >
                  <Trash2 aria-hidden />
                </button>
              </div>
            </div>
          );
        })}
      </div>
      <div className="mt-2 flex flex-col gap-1.5">
        <button type="button" className="ghost" onClick={onAddOpenApp}>
          Add app
        </button>
        <SettingsHelpText>
          Commands receive the selected path as the final argument.{" "}
          {isMacPlatform()
            ? "Apps open via `open -a` with optional args."
            : "Apps run as an executable with optional args."}
        </SettingsHelpText>
      </div>
    </SettingsSection>
  );
}
