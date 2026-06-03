import { pushErrorToast } from "@services/toasts";
import type { Dispatch, SetStateAction } from "react";
import {
  SettingsField,
  SettingsFieldLabel,
  SettingsFieldRow,
  SettingsHelpText,
  SettingsInput,
  SettingsSection,
  SettingsSelect,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { WorkspaceInfo } from "@/types";

type SettingsEnvironmentsSectionProps = {
  mainWorkspaces: WorkspaceInfo[];
  environmentWorkspace: WorkspaceInfo | null;
  environmentSaving: boolean;
  environmentError: string | null;
  environmentDraftScript: string;
  environmentSavedScript: string | null;
  environmentDirty: boolean;
  globalWorktreesFolderDraft: string;
  globalWorktreesFolderSaved: string | null;
  globalWorktreesFolderDirty: boolean;
  worktreesFolderDraft: string;
  worktreesFolderSaved: string | null;
  worktreesFolderDirty: boolean;
  onSetEnvironmentWorkspaceId: Dispatch<SetStateAction<string | null>>;
  onSetEnvironmentDraftScript: Dispatch<SetStateAction<string>>;
  onSetGlobalWorktreesFolderDraft: Dispatch<SetStateAction<string>>;
  onSetWorktreesFolderDraft: Dispatch<SetStateAction<string>>;
  onSaveEnvironmentSetup: () => Promise<void>;
};

export function SettingsEnvironmentsSection({
  mainWorkspaces,
  environmentWorkspace,
  environmentSaving,
  environmentError,
  environmentDraftScript,
  environmentSavedScript,
  environmentDirty,
  globalWorktreesFolderDraft,
  globalWorktreesFolderSaved: _globalWorktreesFolderSaved,
  globalWorktreesFolderDirty,
  worktreesFolderDraft,
  worktreesFolderSaved: _worktreesFolderSaved,
  worktreesFolderDirty,
  onSetEnvironmentWorkspaceId,
  onSetEnvironmentDraftScript,
  onSetGlobalWorktreesFolderDraft,
  onSetWorktreesFolderDraft,
  onSaveEnvironmentSetup,
}: SettingsEnvironmentsSectionProps) {
  const hasAnyChanges = environmentDirty || globalWorktreesFolderDirty || worktreesFolderDirty;
  const hasProjects = mainWorkspaces.length > 0;

  return (
    <SettingsSection
      title="Environments"
      subtitle="Configure per-project setup scripts and worktree locations."
    >
      <SettingsField>
        <SettingsFieldLabel htmlFor="settings-global-worktrees-folder">
          Global worktrees root
        </SettingsFieldLabel>
        <SettingsHelpText>
          Default location for new worktrees when a project does not override it. Each project gets
          its own subfolder under this root.
        </SettingsHelpText>
        <SettingsFieldRow>
          <SettingsInput
            id="settings-global-worktrees-folder"
            type="text"
            value={globalWorktreesFolderDraft}
            onChange={(event) => onSetGlobalWorktreesFolderDraft(event.target.value)}
            placeholder="/path/to/worktrees-root"
            disabled={environmentSaving}
          />
          <button
            type="button"
            className="ghost py-1.5 px-2.5 text-ui-sm"
            onClick={async () => {
              try {
                const { open } = await import("@tauri-apps/plugin-dialog");
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: "Select global worktrees root",
                });
                if (selected && typeof selected === "string") {
                  onSetGlobalWorktreesFolderDraft(selected);
                }
              } catch (error) {
                pushErrorToast({
                  title: "Failed to open folder picker",
                  message: error instanceof Error ? error.message : String(error),
                });
              }
            }}
            disabled={environmentSaving}
          >
            Browse
          </button>
        </SettingsFieldRow>
        {!hasProjects ? (
          <div className="flex gap-2.5 items-center">
            <button
              type="button"
              className="ghost py-1.5 px-2.5 text-ui-sm"
              onClick={() => onSetGlobalWorktreesFolderDraft(_globalWorktreesFolderSaved ?? "")}
              disabled={environmentSaving || !globalWorktreesFolderDirty}
            >
              Reset
            </button>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={() => {
                void onSaveEnvironmentSetup();
              }}
              disabled={environmentSaving || !globalWorktreesFolderDirty}
            >
              {environmentSaving ? "Saving..." : "Save"}
            </button>
          </div>
        ) : null}
        {!hasProjects && environmentError ? (
          <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">
            {environmentError}
          </div>
        ) : null}
      </SettingsField>

      {!hasProjects ? (
        <div className="text-text-faint text-ui-sm">No projects yet.</div>
      ) : (
        <>
          <SettingsField>
            <SettingsFieldLabel htmlFor="settings-environment-project">
              Project
            </SettingsFieldLabel>
            <SettingsSelect
              id="settings-environment-project"
              value={environmentWorkspace?.id ?? ""}
              onChange={(event) => onSetEnvironmentWorkspaceId(event.target.value)}
              disabled={environmentSaving}
            >
              {mainWorkspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </SettingsSelect>
            {environmentWorkspace ? (
              <SettingsHelpText>{environmentWorkspace.path}</SettingsHelpText>
            ) : null}
          </SettingsField>

          <SettingsField>
            <SettingsFieldLabel>Setup script</SettingsFieldLabel>
            <SettingsHelpText>
              Runs once in a dedicated terminal after each new worktree is created.
            </SettingsHelpText>
            {environmentError ? (
              <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">
                {environmentError}
              </div>
            ) : null}
            <textarea
              className="w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
              value={environmentDraftScript}
              onChange={(event) => onSetEnvironmentDraftScript(event.target.value)}
              placeholder="pnpm install"
              spellCheck={false}
              disabled={environmentSaving}
            />
            <div className="flex gap-2.5 items-center">
              <button
                type="button"
                className="ghost py-1.5 px-2.5 text-ui-sm"
                onClick={() => {
                  const clipboard = typeof navigator === "undefined" ? null : navigator.clipboard;
                  if (!clipboard?.writeText) {
                    pushErrorToast({
                      title: "Copy failed",
                      message:
                        "Clipboard access is unavailable in this environment. Copy the script manually instead.",
                    });
                    return;
                  }

                  void clipboard.writeText(environmentDraftScript).catch(() => {
                    pushErrorToast({
                      title: "Copy failed",
                      message:
                        "Could not write to the clipboard. Copy the script manually instead.",
                    });
                  });
                }}
                disabled={environmentSaving || environmentDraftScript.length === 0}
              >
                Copy
              </button>
              <button
                type="button"
                className="ghost py-1.5 px-2.5 text-ui-sm"
                onClick={() => onSetEnvironmentDraftScript(environmentSavedScript ?? "")}
                disabled={environmentSaving || !environmentDirty}
              >
                Reset
              </button>
              <button
                type="button"
                className="primary py-1.5 px-2.5 text-ui-sm"
                onClick={() => {
                  void onSaveEnvironmentSetup();
                }}
                disabled={environmentSaving || !hasAnyChanges}
              >
                {environmentSaving ? "Saving..." : "Save"}
              </button>
            </div>
          </SettingsField>

          <SettingsField>
            <SettingsFieldLabel htmlFor="settings-worktrees-folder">
              Worktrees folder
            </SettingsFieldLabel>
            <SettingsHelpText>
              Custom location for this project&apos;s worktrees. Leave empty to use the global root
              or the built-in default.
            </SettingsHelpText>
            <SettingsFieldRow>
              <SettingsInput
                id="settings-worktrees-folder"
                type="text"
                value={worktreesFolderDraft}
                onChange={(event) => onSetWorktreesFolderDraft(event.target.value)}
                placeholder="/path/to/worktrees"
                disabled={environmentSaving}
              />
              <button
                type="button"
                className="ghost py-1.5 px-2.5 text-ui-sm"
                onClick={async () => {
                  try {
                    const { open } = await import("@tauri-apps/plugin-dialog");
                    const selected = await open({
                      directory: true,
                      multiple: false,
                      title: "Select worktrees folder",
                    });
                    if (selected && typeof selected === "string") {
                      onSetWorktreesFolderDraft(selected);
                    }
                  } catch (error) {
                    pushErrorToast({
                      title: "Failed to open folder picker",
                      message: error instanceof Error ? error.message : String(error),
                    });
                  }
                }}
                disabled={environmentSaving}
              >
                Browse
              </button>
            </SettingsFieldRow>
          </SettingsField>
        </>
      )}
    </SettingsSection>
  );
}
