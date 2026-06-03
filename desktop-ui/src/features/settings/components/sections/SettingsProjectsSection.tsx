import ChevronDown from "lucide-react/dist/esm/icons/chevron-down";
import ChevronUp from "lucide-react/dist/esm/icons/chevron-up";
import Trash2 from "lucide-react/dist/esm/icons/trash-2";
import type { Dispatch, SetStateAction } from "react";
import {
  SettingsInput,
  SettingsSection,
  SettingsSelect,
  SettingsSubsection,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type { WorkspaceGroup, WorkspaceInfo } from "@/types";
import { cn } from "@/utils/cn";

type GroupedWorkspaces = Array<{
  id: string | null;
  name: string;
  workspaces: WorkspaceInfo[];
}>;

type SettingsProjectsSectionProps = {
  workspaceGroups: WorkspaceGroup[];
  groupedWorkspaces: GroupedWorkspaces;
  ungroupedLabel: string;
  groupDrafts: Record<string, string>;
  newGroupName: string;
  groupError: string | null;
  projects: WorkspaceInfo[];
  canCreateGroup: boolean;
  onSetNewGroupName: Dispatch<SetStateAction<string>>;
  onSetGroupDrafts: Dispatch<SetStateAction<Record<string, string>>>;
  onCreateGroup: () => Promise<void>;
  onRenameGroup: (group: WorkspaceGroup) => Promise<void>;
  onMoveWorkspaceGroup: (id: string, direction: "up" | "down") => Promise<boolean | null>;
  onDeleteGroup: (group: WorkspaceGroup) => Promise<void>;
  onChooseGroupCopiesFolder: (group: WorkspaceGroup) => Promise<void>;
  onClearGroupCopiesFolder: (group: WorkspaceGroup) => Promise<void>;
  onAssignWorkspaceGroup: (workspaceId: string, groupId: string | null) => Promise<boolean | null>;
  onMoveWorkspace: (id: string, direction: "up" | "down") => void;
  onDeleteWorkspace: (id: string) => void;
};

export function SettingsProjectsSection({
  workspaceGroups,
  groupedWorkspaces,
  ungroupedLabel,
  groupDrafts,
  newGroupName,
  groupError,
  projects,
  canCreateGroup,
  onSetNewGroupName,
  onSetGroupDrafts,
  onCreateGroup,
  onRenameGroup,
  onMoveWorkspaceGroup,
  onDeleteGroup,
  onChooseGroupCopiesFolder,
  onClearGroupCopiesFolder,
  onAssignWorkspaceGroup,
  onMoveWorkspace,
  onDeleteWorkspace,
}: SettingsProjectsSectionProps) {
  return (
    <SettingsSection
      title="Projects"
      subtitle="Group related workspaces and reorder projects within each group."
    >
      <SettingsSubsection title="Groups" subtitle="Create group labels for related repositories." />
      <div className="flex flex-col gap-2.5">
        <div className="flex items-center gap-2">
          <SettingsInput
            compact
            value={newGroupName}
            placeholder="New group name"
            onChange={(event) => onSetNewGroupName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && canCreateGroup) {
                event.preventDefault();
                void onCreateGroup();
              }
            }}
          />
          <button
            type="button"
            className="ghost py-1.5 px-2.5 text-ui-sm"
            onClick={() => {
              void onCreateGroup();
            }}
            disabled={!canCreateGroup}
          >
            Add group
          </button>
        </div>
        {groupError && <div className="text-ui-xs text-status-error">{groupError}</div>}
        {workspaceGroups.length > 0 ? (
          <div className="flex flex-col gap-2">
            {workspaceGroups.map((group, index) => (
              <div key={group.id} className="flex items-start justify-between gap-2">
                <div className="flex flex-col gap-2">
                  <SettingsInput
                    compact
                    value={groupDrafts[group.id] ?? group.name}
                    onChange={(event) =>
                      onSetGroupDrafts((prev) => ({
                        ...prev,
                        [group.id]: event.target.value,
                      }))
                    }
                    onBlur={() => {
                      void onRenameGroup(group);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        void onRenameGroup(group);
                      }
                    }}
                  />
                  <div className="flex flex-col gap-1.5">
                    <div className="text-ui-xs text-text-faint">Copies folder</div>
                    <div className="flex items-center gap-2">
                      <div
                        className={cn(
                          "flex-1 min-w-0 px-2.5 py-2 rounded-xl border border-border-muted bg-surface-control text-text-strong text-ui-xs whitespace-nowrap overflow-hidden text-ellipsis",
                          !group.copiesFolder && "text-text-faint",
                        )}
                        title={group.copiesFolder ?? ""}
                      >
                        {group.copiesFolder ?? "Not set"}
                      </div>
                      <button
                        type="button"
                        className="ghost py-1.5 px-2.5 text-ui-sm"
                        onClick={() => {
                          void onChooseGroupCopiesFolder(group);
                        }}
                      >
                        Choose…
                      </button>
                      <button
                        type="button"
                        className="ghost py-1.5 px-2.5 text-ui-sm"
                        onClick={() => {
                          void onClearGroupCopiesFolder(group);
                        }}
                        disabled={!group.copiesFolder}
                      >
                        Clear
                      </button>
                    </div>
                  </div>
                </div>
                <div className="inline-flex items-center gap-1.5">
                  <button
                    type="button"
                    className="ghost icon-button"
                    onClick={() => {
                      void onMoveWorkspaceGroup(group.id, "up");
                    }}
                    disabled={index === 0}
                    aria-label="Move group up"
                  >
                    <ChevronUp aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="ghost icon-button"
                    onClick={() => {
                      void onMoveWorkspaceGroup(group.id, "down");
                    }}
                    disabled={index === workspaceGroups.length - 1}
                    aria-label="Move group down"
                  >
                    <ChevronDown aria-hidden />
                  </button>
                  <button
                    type="button"
                    className="ghost icon-button"
                    onClick={() => {
                      void onDeleteGroup(group);
                    }}
                    aria-label="Delete group"
                  >
                    <Trash2 aria-hidden />
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-text-faint text-ui-sm">No groups yet.</div>
        )}
      </div>
      <SettingsSubsection
        title="Projects"
        subtitle="Assign projects to groups and adjust their order."
      />
      <div className="flex flex-col gap-2.5">
        {groupedWorkspaces.map((group) => (
          <div key={group.id ?? "ungrouped"} className="flex flex-col gap-2.5 first:mt-0 mt-3">
            <div className="uppercase text-ui-xs tracking-widest text-text-faint pl-1">
              {group.name}
            </div>
            {group.workspaces.map((workspace, index) => {
              const groupValue = workspaceGroups.some(
                (entry) => entry.id === workspace.settings.groupId,
              )
                ? (workspace.settings.groupId ?? "")
                : "";
              return (
                <div
                  key={workspace.id}
                  className="flex items-center justify-between gap-3 p-3 px-3.5 rounded-xl bg-surface-card border border-border-muted"
                >
                  <div className="flex flex-col gap-1 min-w-0">
                    <div className="text-ui-sm font-semibold text-text-strong">{workspace.name}</div>
                    <div className="text-ui-xs text-text-subtle">{workspace.path}</div>
                  </div>
                  <div className="inline-flex gap-1.5">
                    <SettingsSelect
                      className="py-1.5 px-2 text-ui-xs"
                      value={groupValue}
                      onChange={(event) => {
                        const nextGroupId = event.target.value || null;
                        void onAssignWorkspaceGroup(workspace.id, nextGroupId);
                      }}
                    >
                      <option value="">{ungroupedLabel}</option>
                      {workspaceGroups.map((entry) => (
                        <option key={entry.id} value={entry.id}>
                          {entry.name}
                        </option>
                      ))}
                    </SettingsSelect>
                    <button
                      type="button"
                      className="ghost icon-button"
                      onClick={() => onMoveWorkspace(workspace.id, "up")}
                      disabled={index === 0}
                      aria-label="Move project up"
                    >
                      <ChevronUp aria-hidden />
                    </button>
                    <button
                      type="button"
                      className="ghost icon-button"
                      onClick={() => onMoveWorkspace(workspace.id, "down")}
                      disabled={index === group.workspaces.length - 1}
                      aria-label="Move project down"
                    >
                      <ChevronDown aria-hidden />
                    </button>
                    <button
                      type="button"
                      className="ghost icon-button"
                      onClick={() => onDeleteWorkspace(workspace.id)}
                      aria-label="Delete project"
                    >
                      <Trash2 aria-hidden />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        ))}
        {projects.length === 0 && <div className="text-text-faint text-ui-sm">No projects yet.</div>}
      </div>
    </SettingsSection>
  );
}
