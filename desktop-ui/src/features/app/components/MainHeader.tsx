import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { revealInFileManagerLabel } from "@utils/platformPaths";
import Check from "lucide-react/dist/esm/icons/check";
import Copy from "lucide-react/dist/esm/icons/copy";
import Terminal from "lucide-react/dist/esm/icons/terminal";
import type { ReactNode } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  MenuTrigger,
  PopoverSurface,
} from "@/features/design-system/components/popover/PopoverPrimitives";
import { BranchList } from "@/features/git/components/BranchList";
import { filterBranches, findExactBranch } from "@/features/git/utils/branchSearch";
import { validateBranchName } from "@/features/git/utils/branchValidation";
import type { BranchInfo, OpenAppTarget, WorkspaceInfo } from "@/types";
import { cn } from "@/utils/cn";
import { useMenuController } from "../hooks/useMenuController";
import type { WorkspaceLaunchScriptsState } from "../hooks/useWorkspaceLaunchScripts";
import { LaunchScriptButton } from "./LaunchScriptButton";
import { LaunchScriptEntryButton } from "./LaunchScriptEntryButton";
import { OpenAppMenu } from "./OpenAppMenu";

type MainHeaderProps = {
  workspace: WorkspaceInfo;
  parentName?: string | null;
  worktreeLabel?: string | null;
  disableBranchMenu?: boolean;
  parentPath?: string | null;
  worktreePath?: string | null;
  openTargets: OpenAppTarget[];
  openAppIconById: Record<string, string>;
  selectedOpenAppId: string;
  onSelectOpenAppId: (id: string) => void;
  branchName: string;
  branches: BranchInfo[];
  onCheckoutBranch: (name: string) => Promise<void> | void;
  onCreateBranch: (name: string) => Promise<void> | void;
  canCopyThread?: boolean;
  onCopyThread?: () => void | Promise<void>;
  onToggleTerminal: () => void;
  isTerminalOpen: boolean;
  showTerminalButton?: boolean;
  showWorkspaceTools?: boolean;
  extraActionsNode?: ReactNode;
  launchScript?: string | null;
  launchScriptEditorOpen?: boolean;
  launchScriptDraft?: string;
  launchScriptSaving?: boolean;
  launchScriptError?: string | null;
  onRunLaunchScript?: () => void;
  onOpenLaunchScriptEditor?: () => void;
  onCloseLaunchScriptEditor?: () => void;
  onLaunchScriptDraftChange?: (value: string) => void;
  onSaveLaunchScript?: () => void;
  launchScriptsState?: WorkspaceLaunchScriptsState;
  worktreeRename?: {
    name: string;
    error: string | null;
    notice: string | null;
    isSubmitting: boolean;
    isDirty: boolean;
    upstream?: {
      oldBranch: string;
      newBranch: string;
      error: string | null;
      isSubmitting: boolean;
      onConfirm: () => void;
    } | null;
    onFocus: () => void;
    onChange: (value: string) => void;
    onCancel: () => void;
    onCommit: () => void;
  };
};

export function MainHeader({
  workspace,
  parentName = null,
  worktreeLabel = null,
  disableBranchMenu = false,
  parentPath = null,
  worktreePath = null,
  openTargets,
  openAppIconById,
  selectedOpenAppId,
  onSelectOpenAppId,
  branchName,
  branches,
  onCheckoutBranch,
  onCreateBranch,
  canCopyThread = false,
  onCopyThread,
  onToggleTerminal,
  isTerminalOpen,
  showTerminalButton = true,
  showWorkspaceTools = true,
  extraActionsNode,
  launchScript = null,
  launchScriptEditorOpen = false,
  launchScriptDraft = "",
  launchScriptSaving = false,
  launchScriptError = null,
  onRunLaunchScript,
  onOpenLaunchScriptEditor,
  onCloseLaunchScriptEditor,
  onLaunchScriptDraftChange,
  onSaveLaunchScript,
  launchScriptsState,
  worktreeRename,
}: MainHeaderProps) {
  const [branchQuery, setBranchQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copyFeedback, setCopyFeedback] = useState(false);
  const copyTimeoutRef = useRef<number | null>(null);
  const renameInputRef = useRef<HTMLInputElement | null>(null);
  const renameConfirmRef = useRef<HTMLButtonElement | null>(null);
  const renameOnCancel = worktreeRename?.onCancel;
  const branchMenu = useMenuController({
    onDismiss: () => {
      setBranchQuery("");
      setError(null);
    },
  });
  const infoMenu = useMenuController();
  const { isOpen: menuOpen, setOpen: setMenuOpen, containerRef: menuRef } = branchMenu;
  const { isOpen: infoOpen, containerRef: infoRef } = infoMenu;

  const trimmedQuery = branchQuery.trim();
  const filteredBranches = useMemo(
    () => filterBranches(branches, branchQuery, { mode: "includes", whenEmptyLimit: 12 }),
    [branches, branchQuery],
  );
  const exactMatch = useMemo(
    () => findExactBranch(branches, trimmedQuery),
    [branches, trimmedQuery],
  );
  const canCreate = trimmedQuery.length > 0 && !exactMatch;
  const branchValidationMessage = useMemo(() => validateBranchName(trimmedQuery), [trimmedQuery]);
  const resolvedWorktreePath = worktreePath ?? workspace.path;
  const relativeWorktreePath = useMemo(() => {
    if (!parentPath) {
      return resolvedWorktreePath;
    }
    return resolvedWorktreePath.startsWith(`${parentPath}/`)
      ? resolvedWorktreePath.slice(parentPath.length + 1)
      : resolvedWorktreePath;
  }, [parentPath, resolvedWorktreePath]);
  const cdCommand = useMemo(() => `cd "${relativeWorktreePath}"`, [relativeWorktreePath]);

  useEffect(() => {
    if (!infoOpen && renameOnCancel) {
      renameOnCancel();
    }
  }, [infoOpen, renameOnCancel]);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        window.clearTimeout(copyTimeoutRef.current);
      }
    };
  }, []);

  const handleCopyClick = async () => {
    if (!onCopyThread) {
      return;
    }
    try {
      await onCopyThread();
      setCopyFeedback(true);
      if (copyTimeoutRef.current) {
        window.clearTimeout(copyTimeoutRef.current);
      }
      copyTimeoutRef.current = window.setTimeout(() => {
        setCopyFeedback(false);
      }, 1200);
    } catch {
      // Errors are handled upstream in the copy handler.
    }
  };

  return (
    <header
      className="flex justify-between items-center gap-5 min-w-0 flex-1 w-full select-none"
      data-tauri-drag-region
    >
      <div className="flex items-center min-w-0 flex-1 max-w-[min(100%,var(--conversation-column-width))]">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-ui-sm font-semibold text-text-primary truncate">
            {parentName ? parentName : workspace.name}
          </span>
          <span className="text-text-muted text-ui-sm" aria-hidden>
            ›
          </span>
          {disableBranchMenu ? (
            <div className="flex items-center" ref={infoRef}>
              <MenuTrigger
                isOpen={infoOpen}
                popupRole="dialog"
                className="text-ui-sm text-text-muted bg-transparent border-none cursor-pointer"
                onClick={infoMenu.toggle}
                data-tauri-drag-region="false"
                title="Worktree info"
              >
                {worktreeLabel || branchName}
              </MenuTrigger>
              {infoOpen && (
                <PopoverSurface className="min-w-[280px] p-3" role="dialog">
                  {worktreeRename && (
                    <div className="mb-3">
                      <span className="text-ui-xs text-text-muted uppercase tracking-wider block mb-1">
                        Name
                      </span>
                      <div className="flex items-center gap-2">
                        <input
                          ref={renameInputRef}
                          className="flex-1 bg-surface-control border border-border-muted rounded-lg px-2 py-1 text-ui-sm text-text-primary outline-none [-webkit-app-region:no-drag]"
                          value={worktreeRename.name}
                          onFocus={() => {
                            worktreeRename.onFocus();
                            renameInputRef.current?.select();
                          }}
                          onChange={(event) => worktreeRename.onChange(event.target.value)}
                          onBlur={(event) => {
                            const nextTarget = event.relatedTarget as Node | null;
                            if (
                              renameConfirmRef.current &&
                              nextTarget &&
                              renameConfirmRef.current.contains(nextTarget)
                            ) {
                              return;
                            }
                            if (!worktreeRename.isSubmitting && worktreeRename.isDirty) {
                              worktreeRename.onCommit();
                            }
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Escape") {
                              event.preventDefault();
                              if (!worktreeRename.isSubmitting) {
                                worktreeRename.onCancel();
                              }
                            }
                            if (event.key === "Enter" && !worktreeRename.isSubmitting) {
                              event.preventDefault();
                              worktreeRename.onCommit();
                            }
                          }}
                          data-tauri-drag-region="false"
                          disabled={worktreeRename.isSubmitting}
                        />
                        <button
                          type="button"
                          className="icon-button"
                          ref={renameConfirmRef}
                          onClick={() => worktreeRename.onCommit()}
                          disabled={worktreeRename.isSubmitting || !worktreeRename.isDirty}
                          aria-label="Confirm rename"
                          title="Confirm rename"
                        >
                          <Check aria-hidden />
                        </button>
                      </div>
                      {worktreeRename.error && (
                        <div className="text-ui-xs text-status-error mt-1">{worktreeRename.error}</div>
                      )}
                      {worktreeRename.notice && (
                        <span className="text-ui-xs text-text-muted">{worktreeRename.notice}</span>
                      )}
                      {worktreeRename.upstream && (
                        <div className="mt-2 p-2 rounded-lg bg-surface-card">
                          <span className="text-ui-xs text-text-muted">
                            Do you want to update the upstream branch to{" "}
                            <strong>{worktreeRename.upstream.newBranch}</strong>?
                          </span>
                          <button
                            type="button"
                            className="ghost mt-2"
                            onClick={worktreeRename.upstream.onConfirm}
                            disabled={worktreeRename.upstream.isSubmitting}
                          >
                            Update upstream
                          </button>
                          {worktreeRename.upstream.error && (
                            <div className="text-ui-xs text-status-error mt-1">
                              {worktreeRename.upstream.error}
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                  <div className="text-ui-sm font-semibold text-text-stronger mb-2">Worktree</div>
                  <div className="mb-3">
                    <span className="text-ui-xs text-text-muted uppercase tracking-wider block mb-1">
                      Terminal{parentPath ? " (repo root)" : ""}
                    </span>
                    <div className="flex items-center gap-2">
                      <code className="font-code text-ui-xs text-text-primary bg-surface-control px-2 py-1 rounded-md">
                        {cdCommand}
                      </code>
                      <button
                        type="button"
                        className="text-text-muted hover:text-text-strong [-webkit-app-region:no-drag]"
                        onClick={async () => {
                          await navigator.clipboard.writeText(cdCommand);
                        }}
                        data-tauri-drag-region="false"
                        aria-label="Copy command"
                        title="Copy command"
                      >
                        <Copy aria-hidden />
                      </button>
                    </div>
                    <span className="text-ui-xs text-text-muted block mt-1">
                      Open this worktree in your terminal.
                    </span>
                  </div>
                  <div className="mb-0">
                    <span className="text-ui-xs text-text-muted uppercase tracking-wider block mb-1">
                      Reveal
                    </span>
                    <button
                      type="button"
                      className="text-text-accent hover:underline text-ui-sm [-webkit-app-region:no-drag]"
                      onClick={async () => {
                        await revealItemInDir(resolvedWorktreePath);
                      }}
                      data-tauri-drag-region="false"
                    >
                      {revealInFileManagerLabel()}
                    </button>
                  </div>
                </PopoverSurface>
              )}
            </div>
          ) : (
            <div className="relative" ref={menuRef}>
              <MenuTrigger
                isOpen={menuOpen}
                className="flex items-center gap-1 bg-transparent border-none cursor-pointer text-text-muted hover:text-text-strong"
                onClick={branchMenu.toggle}
                data-tauri-drag-region="false"
              >
                <span className="text-ui-sm text-text-muted">{branchName}</span>
                <span className="text-ui-xs text-text-muted" aria-hidden>
                  ›
                </span>
              </MenuTrigger>
              {menuOpen && (
                <PopoverSurface
                  className="absolute left-0 top-[calc(100%+8px)] min-w-[200px] z-5"
                  role="menu"
                  data-tauri-drag-region="false"
                >
                  <div className="p-2">
                    <div className="flex gap-2 mb-2">
                      <input
                        value={branchQuery}
                        onChange={(event) => {
                          setBranchQuery(event.target.value);
                          setError(null);
                        }}
                        onKeyDown={async (event) => {
                          if (event.key !== "Enter") {
                            return;
                          }
                          event.preventDefault();
                          if (branchValidationMessage) {
                            setError(branchValidationMessage);
                            return;
                          }
                          if (canCreate) {
                            try {
                              await onCreateBranch(trimmedQuery);
                              setMenuOpen(false);
                              setBranchQuery("");
                              setError(null);
                            } catch (err) {
                              setError(err instanceof Error ? err.message : String(err));
                            }
                            return;
                          }
                          if (exactMatch && exactMatch.name !== branchName) {
                            try {
                              await onCheckoutBranch(exactMatch.name);
                              setMenuOpen(false);
                              setBranchQuery("");
                              setError(null);
                            } catch (err) {
                              setError(err instanceof Error ? err.message : String(err));
                            }
                          }
                        }}
                        placeholder="Search or create branch"
                        className="flex-1 bg-surface-control border border-border-muted rounded-lg px-2 py-1 text-ui-sm text-text-primary outline-none [-webkit-app-region:no-drag]"
                        autoCorrect="off"
                        autoCapitalize="none"
                        spellCheck={false}
                        data-tauri-drag-region="false"
                        aria-label="Search branches"
                      />
                      <button
                        type="button"
                        className="primary"
                        disabled={!canCreate || Boolean(branchValidationMessage)}
                        onClick={async () => {
                          if (branchValidationMessage) {
                            setError(branchValidationMessage);
                            return;
                          }
                          if (!canCreate) {
                            return;
                          }
                          try {
                            await onCreateBranch(trimmedQuery);
                            setMenuOpen(false);
                            setBranchQuery("");
                            setError(null);
                          } catch (err) {
                            setError(err instanceof Error ? err.message : String(err));
                          }
                        }}
                        data-tauri-drag-region="false"
                      >
                        Create
                      </button>
                    </div>
                    {branchValidationMessage && (
                      <div className="text-ui-xs text-status-error mt-1">{branchValidationMessage}</div>
                    )}
                    {canCreate && !branchValidationMessage && (
                      <div className="text-ui-xs text-text-muted mt-1">
                        Create branch &ldquo;{trimmedQuery}&rdquo;
                      </div>
                    )}
                  </div>
                  <BranchList
                    branches={filteredBranches}
                    currentBranch={branchName}
                    listClassName="flex flex-col gap-0.5"
                    listRole="none"
                    itemClassName="flex items-center gap-2 px-2 py-1 rounded-md text-ui-sm text-text-muted cursor-pointer hover:bg-surface-hover"
                    currentItemClassName="bg-surface-active text-text-strong"
                    itemRole="menuitem"
                    itemDataTauriDragRegion="false"
                    emptyClassName="p-2 text-ui-xs text-text-muted text-center"
                    emptyText="No branches found"
                    onSelect={async (branch) => {
                      if (branch.name === branchName) {
                        return;
                      }
                      try {
                        await onCheckoutBranch(branch.name);
                        setMenuOpen(false);
                        setBranchQuery("");
                        setError(null);
                      } catch (err) {
                        setError(err instanceof Error ? err.message : String(err));
                      }
                    }}
                  />
                  {error && <div className="text-ui-xs text-status-error mt-1 p-2">{error}</div>}
                </PopoverSurface>
              )}
            </div>
          )}
        </div>
      </div>
      <div className="flex items-center gap-2.5 shrink-0 [-webkit-app-region:no-drag]">
        {showWorkspaceTools &&
          onRunLaunchScript &&
          onOpenLaunchScriptEditor &&
          onCloseLaunchScriptEditor &&
          onLaunchScriptDraftChange &&
          onSaveLaunchScript && (
            <div className="inline-flex items-center gap-1">
              <LaunchScriptButton
                launchScript={launchScript}
                editorOpen={launchScriptEditorOpen}
                draftScript={launchScriptDraft}
                isSaving={launchScriptSaving}
                error={launchScriptError}
                onRun={onRunLaunchScript}
                onOpenEditor={onOpenLaunchScriptEditor}
                onCloseEditor={onCloseLaunchScriptEditor}
                onDraftChange={onLaunchScriptDraftChange}
                onSave={onSaveLaunchScript}
                showNew={Boolean(launchScriptsState)}
                newEditorOpen={launchScriptsState?.newEditorOpen}
                newDraftScript={launchScriptsState?.newDraftScript}
                newDraftIcon={launchScriptsState?.newDraftIcon}
                newDraftLabel={launchScriptsState?.newDraftLabel}
                newError={launchScriptsState?.newError ?? null}
                onOpenNew={launchScriptsState?.onOpenNew}
                onCloseNew={launchScriptsState?.onCloseNew}
                onNewDraftChange={launchScriptsState?.onNewDraftScriptChange}
                onNewDraftIconChange={launchScriptsState?.onNewDraftIconChange}
                onNewDraftLabelChange={launchScriptsState?.onNewDraftLabelChange}
                onCreateNew={launchScriptsState?.onCreateNew}
              />
              {launchScriptsState?.launchScripts.map((entry) => (
                <LaunchScriptEntryButton
                  key={entry.id}
                  entry={entry}
                  editorOpen={launchScriptsState.editorOpenId === entry.id}
                  draftScript={launchScriptsState.draftScript}
                  draftIcon={launchScriptsState.draftIcon}
                  draftLabel={launchScriptsState.draftLabel}
                  isSaving={launchScriptsState.isSaving}
                  error={launchScriptsState.errorById[entry.id] ?? null}
                  onRun={() => launchScriptsState.onRunScript(entry.id)}
                  onOpenEditor={() => launchScriptsState.onOpenEditor(entry.id)}
                  onCloseEditor={launchScriptsState.onCloseEditor}
                  onDraftChange={launchScriptsState.onDraftScriptChange}
                  onDraftIconChange={launchScriptsState.onDraftIconChange}
                  onDraftLabelChange={launchScriptsState.onDraftLabelChange}
                  onSave={launchScriptsState.onSaveScript}
                  onDelete={launchScriptsState.onDeleteScript}
                />
              ))}
            </div>
          )}
        {showWorkspaceTools ? (
          <OpenAppMenu
            path={resolvedWorktreePath}
            openTargets={openTargets}
            selectedOpenAppId={selectedOpenAppId}
            onSelectOpenAppId={onSelectOpenAppId}
            iconById={openAppIconById}
          />
        ) : null}
        {showTerminalButton && (
          <button
            type="button"
            className={cn(
              "ghost main-header-action ds-tooltip-trigger",
              isTerminalOpen && "is-active"
            )}
            onClick={onToggleTerminal}
            data-tauri-drag-region="false"
            aria-label="Toggle terminal panel"
            title="Terminal"
            data-tooltip="Terminal"
            data-tooltip-placement="bottom"
          >
            <Terminal size={14} aria-hidden />
          </button>
        )}
        <button
          type="button"
          className={cn(
            "ghost main-header-action ds-tooltip-trigger",
            copyFeedback && "is-copied"
          )}
          onClick={handleCopyClick}
          disabled={!canCopyThread || !onCopyThread}
          data-tauri-drag-region="false"
          aria-label="Copy thread"
          title="Copy thread"
          data-tooltip="Copy thread"
          data-tooltip-placement="bottom"
        >
          <span className="relative w-3.5 h-3.5 inline-flex items-center justify-center" aria-hidden>
            <Copy
              className={cn(
                "absolute inset-0 transition-all duration-150",
                copyFeedback ? "opacity-0 scale-[0.82] blur-[2px]" : "opacity-100 scale-100 blur-0"
              )}
              size={14}
            />
            <Check
              className={cn(
                "absolute inset-0 transition-all duration-150",
                copyFeedback ? "opacity-100 scale-100 blur-0" : "opacity-0 scale-[0.82] blur-[2px]"
              )}
              size={14}
            />
          </span>
        </button>
        {extraActionsNode}
      </div>
    </header>
  );
}
