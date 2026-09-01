import { useAppBootstrapOrchestration } from "@app/bootstrap/useAppBootstrapOrchestration";
import { MainAppShell } from "@app/components/MainAppShell";
import { AppView } from "@app/constants/appViews";
import { useArchiveShortcut } from "@app/hooks/useArchiveShortcut";
import { useInterruptShortcut } from "@app/hooks/useInterruptShortcut";
import { useLayoutController } from "@app/hooks/useLayoutController";
import { useMainAppComposerWorkspaceState } from "@app/hooks/useMainAppComposerWorkspaceState";
import { useMainAppDisplayNodes } from "@app/hooks/useMainAppDisplayNodes";
import { useMainAppGitState } from "@app/hooks/useMainAppGitState";
import { useMainAppLayoutNodes } from "@app/hooks/useMainAppLayoutNodes";
import { useMainAppLayoutSurfaces } from "@app/hooks/useMainAppLayoutSurfaces";
import { useMainAppModals } from "@app/hooks/useMainAppModals";
import { useMainAppPromptActions } from "@app/hooks/useMainAppPromptActions";
import { useMainAppSettingsActions } from "@app/hooks/useMainAppSettingsActions";
import { useMainAppShellProps } from "@app/hooks/useMainAppShellProps";
import { useMainAppSidebarMenuOrchestration } from "@app/hooks/useMainAppSidebarMenuOrchestration";
import { useMainAppThreadCodexState } from "@app/hooks/useMainAppThreadCodexState";
import { useMainAppWorkspaceActions } from "@app/hooks/useMainAppWorkspaceActions";
import { useMainAppWorkspaceLifecycle } from "@app/hooks/useMainAppWorkspaceLifecycle";
import { useMainAppWorktreeState } from "@app/hooks/useMainAppWorktreeState";
import { useNewAgentDraft } from "@app/hooks/useNewAgentDraft";
import { useOpenAppIcons } from "@app/hooks/useOpenAppIcons";
import { usePlanReadyActions } from "@app/hooks/usePlanReadyActions";
import { useRemoteThreadLiveConnection } from "@app/hooks/useRemoteThreadLiveConnection";
import { useResponseRequiredNotificationsController } from "@app/hooks/useResponseRequiredNotificationsController";
import { useSystemNotificationThreadLinks } from "@app/hooks/useSystemNotificationThreadLinks";
import { useTauriEvent } from "@app/hooks/useTauriEvent";
import { useThreadListActions } from "@app/hooks/useThreadListActions";
import { useThreadListSortKey } from "@app/hooks/useThreadListSortKey";
import { useThreadRows } from "@app/hooks/useThreadRows";
import { useTrayRecentThreads } from "@app/hooks/useTrayRecentThreads";
import { useUpdaterController } from "@app/hooks/useUpdaterController";
import { useWorkspaceController } from "@app/hooks/useWorkspaceController";
import { useWorkspaceLaunchScript } from "@app/hooks/useWorkspaceLaunchScript";
import { useWorkspaceLaunchScripts } from "@app/hooks/useWorkspaceLaunchScripts";
import { useWorktreeSetupScript } from "@app/hooks/useWorktreeSetupScript";
import { useAppShellOrchestration } from "@app/orchestration/useLayoutOrchestration";
import {
  useThreadCodexBootstrapOrchestration,
  useThreadCodexSyncOrchestration,
  useThreadSelectionHandlersOrchestration,
  useThreadUiOrchestration,
} from "@app/orchestration/useThreadOrchestration";
import {
  useWorkspaceInsightsOrchestration,
  useWorkspaceOrderingOrchestration,
} from "@app/orchestration/useWorkspaceOrchestration";
import { subscribeTrayOpenThread } from "@services/events";
import { StuckThreadBanner } from "@threads/components/StuckThreadBanner";
import { useCopyThread } from "@threads/hooks/useCopyThread";
import { useStuckThreadDetector } from "@threads/hooks/useStuckThreadDetector";
import { useThreads } from "@threads/hooks/useThreads";
import { useThreadWatchdog } from "@threads/hooks/useThreadWatchdog";
import { lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import errorSoundUrl from "@/assets/error-notification.mp3";
import successSoundUrl from "@/assets/success-notification.mp3";
import { useApps } from "@/features/apps/hooks/useApps";
import { ChatErrorBanner } from "@/features/chat/components/ChatErrorBanner";
import { ProviderSetupModal } from "@/features/chat/components/ProviderSetupModal";
import { ProviderSetupPrompt } from "@/features/chat/components/ProviderSetupPrompt";
import { useChatThreads } from "@/features/chat/hooks/useChatThreads";
import { useKlyntbotSurfaceProps } from "@/features/chat/hooks/useKlyntbotSurfaceProps";
import { useCollaborationModeSelection } from "@/features/collaboration/hooks/useCollaborationModeSelection";
import { useCollaborationModes } from "@/features/collaboration/hooks/useCollaborationModes";
import { useComposerEditorState } from "@/features/composer/hooks/useComposerEditorState";
import { useComposerMenuActions } from "@/features/composer/hooks/useComposerMenuActions";
import { useComposerShortcuts } from "@/features/composer/hooks/useComposerShortcuts";
import { Dashboard } from "@/features/dashboard";
import { useAutoExitEmptyDiff } from "@/features/git/hooks/useAutoExitEmptyDiff";
import { useBranchSwitcherShortcut } from "@/features/git/hooks/useBranchSwitcherShortcut";
import { usePullRequestComposer } from "@/features/git/hooks/usePullRequestComposer";
import { effectiveCommitMessageModelId } from "@/features/git/utils/commitMessageModelSelection";
import { isMissingRepo } from "@/features/git/utils/repoErrors";
import { useMobileServerSetup } from "@/features/mobile/hooks/useMobileServerSetup";
import { useModels } from "@/features/models/hooks/useModels";
import { useProviders } from "@/features/models/hooks/useProviders";
import { useErrorToasts } from "@/features/notifications/hooks/useErrorToasts";
import { useCustomPrompts } from "@/features/prompts/hooks/useCustomPrompts";
import { useSkills } from "@/features/skills/hooks/useSkills";
import { useTerminalController } from "@/features/terminal/hooks/useTerminalController";
import { useRenameWorktreePrompt } from "@/features/workspaces/hooks/useRenameWorktreePrompt";
import { useWorkspaceFromUrlPrompt } from "@/features/workspaces/hooks/useWorkspaceFromUrlPrompt";
import { useWorkspaceSelection } from "@/features/workspaces/hooks/useWorkspaceSelection";
import type { ComposerEditorSettings, WorkspaceInfo } from "@/types";
import { normalizeCodexArgsInput } from "@/utils/codexArgsInput";

const SettingsView = lazy(() =>
  import("@settings/components/SettingsView").then((module) => ({
    default: module.SettingsView,
  })),
);

const SettingsShellModal = lazy(() =>
  import("@settings/components/SettingsShellModal").then((module) => ({
    default: module.SettingsShellModal,
  })),
);

const showNewSettings =
  typeof window !== "undefined" && window.localStorage.getItem("klynt-new-settings") === "1";

export default function MainApp() {
  const {
    appSettings,
    setAppSettings,
    doctor,
    codexUpdate,
    appSettingsLoading,
    reduceTransparency,
    setReduceTransparency,
    scaleShortcutTitle,
    scaleShortcutText,
    queueSaveSettings,
    dictationModel,
    dictationState,
    dictationLevel,
    dictationTranscript,
    dictationError,
    dictationHint,
    dictationReady,
    handleToggleDictation,
    cancelDictation,
    clearDictationTranscript,
    clearDictationError,
    clearDictationHint,
    debugOpen,
    setDebugOpen,
    debugEntries,
    showDebugButton,
    addDebugEntry,
    handleCopyDebug,
    clearDebugEntries,
    shouldReduceTransparency,
  } = useAppBootstrapOrchestration();
  const {
    threadListSortKey,
    setThreadListSortKey,
    threadListOrganizeMode,
    setThreadListOrganizeMode,
  } = useThreadListSortKey();
  const activeTab = "codex";
  const setActiveTab = useCallback(
    (
      _tab:
        | "home"
        | "projects"
        | "codex"
        | "git"
        | "log"
        | ((
            prev: "home" | "projects" | "codex" | "git" | "log",
          ) => "home" | "projects" | "codex" | "git" | "log"),
    ) => {},
    [],
  );
  const tabletTab = "codex";
  const {
    workspaces,
    workspaceGroups,
    groupedWorkspaces,
    getWorkspaceGroupName,
    ungroupedLabel,
    activeWorkspace,
    activeWorkspaceId,
    setActiveWorkspaceId,
    addWorkspace,
    addWorkspaceFromPath,
    addWorkspaceFromGitUrl,
    addWorkspacesFromPaths,
    mobileRemoteWorkspacePathPrompt,
    updateMobileRemoteWorkspacePathInput,
    appendMobileRemoteWorkspacePathFromRecent,
    cancelMobileRemoteWorkspacePathPrompt,
    submitMobileRemoteWorkspacePathPrompt,
    addCloneAgent,
    addWorktreeAgent,
    connectWorkspace,
    markWorkspaceConnected,
    updateWorkspaceSettings,
    createWorkspaceGroup,
    renameWorkspaceGroup,
    moveWorkspaceGroup,
    deleteWorkspaceGroup,
    assignWorkspaceGroup,
    removeWorkspace,
    removeWorktree,
    renameWorktree,
    renameWorktreeUpstream,
    deletingWorktreeIds,
    hasLoaded,
    refreshWorkspaces,
  } = useWorkspaceController({
    appSettings,
    addDebugEntry,
    queueSaveSettings,
  });
  const {
    isMobileRuntime,
    showMobileSetupWizard,
    mobileSetupWizardProps,
    handleMobileConnectSuccess,
  } = useMobileServerSetup({
    appSettings,
    appSettingsLoading,
    queueSaveSettings,
    refreshWorkspaces,
  });
  const updaterEnabled = !isMobileRuntime;

  const workspacesById = useMemo(
    () => new Map(workspaces.map((workspace) => [workspace.id, workspace])),
    [workspaces],
  );
  const {
    threadCodexParamsVersion,
    getThreadCodexParams,
    patchThreadCodexParams,
    accessMode,
    setAccessMode,
    preferredModelId,
    setPreferredModelId,
    preferredEffort,
    setPreferredEffort,
    preferredCollabModeId,
    setPreferredCollabModeId,
    preferredCodexArgsOverride,
    setPreferredCodexArgsOverride,
    threadCodexSelectionKey,
    setThreadCodexSelectionKey,
    activeThreadIdRef,
    pendingNewThreadSeedRef,
    persistThreadCodexParams,
  } = useThreadCodexBootstrapOrchestration({
    activeWorkspaceId,
  });
  const {
    appRef,
    isResizing,
    sidebarWidth,
    chatDiffSplitPositionPercent,
    rightPanelWidth,
    onSidebarResizeStart,
    onChatDiffSplitPositionResizeStart,
    onRightPanelResizeStart,
    planPanelHeight,
    onPlanPanelResizeStart,
    terminalPanelHeight,
    onTerminalPanelResizeStart,
    debugPanelHeight,
    onDebugPanelResizeStart,
    isCompact,
    isTablet,
    sidebarCollapsed,
    rightPanelCollapsed,
    collapseSidebar,
    expandSidebar,
    collapseRightPanel,
    expandRightPanel,
    terminalOpen,
    handleDebugClick,
    handleToggleTerminal,
    openTerminal,
    closeTerminal: closeTerminalPanel,
  } = useLayoutController({
    activeWorkspaceId,
    setActiveTab,
    setDebugOpen,
    toggleDebugPanelShortcut: appSettings.toggleDebugPanelShortcut,
    toggleTerminalShortcut: appSettings.toggleTerminalShortcut,
  });
  const sidebarToggleProps = useMemo(
    () => ({
      isCompact,
      sidebarCollapsed,
      rightPanelCollapsed,
      onCollapseSidebar: collapseSidebar,
      onExpandSidebar: expandSidebar,
      onCollapseRightPanel: collapseRightPanel,
      onExpandRightPanel: expandRightPanel,
    }),
    [
      isCompact,
      sidebarCollapsed,
      rightPanelCollapsed,
      collapseSidebar,
      expandSidebar,
      collapseRightPanel,
      expandRightPanel,
    ],
  );
  const composerInputRef = useRef<HTMLTextAreaElement | null>(null);
  const workspaceHomeTextareaRef = useRef<HTMLTextAreaElement | null>(null);

  const getWorkspaceName = useCallback(
    (workspaceId: string) => workspacesById.get(workspaceId)?.name,
    [workspacesById],
  );

  const recordPendingThreadLinkRef = useRef<(workspaceId: string, threadId: string) => void>(
    () => {},
  );

  const { errorToasts, dismissErrorToast } = useErrorToasts();
  const queueGitStatusRefreshRef = useRef<() => void>(() => {});
  const handleThreadMessageActivity = useCallback(() => {
    queueGitStatusRefreshRef.current();
  }, []);

  // Access mode is thread-scoped (best-effort persisted) and falls back to the app default.

  const {
    models,
    selectedModel,
    selectedModelId,
    setSelectedModelId,
    reasoningSupported,
    reasoningOptions,
    selectedEffort,
    setSelectedEffort,
    refreshModels,
  } = useModels({
    activeWorkspace,
    onDebug: addDebugEntry,
    preferredModelId,
    preferredEffort,
    selectionKey: threadCodexSelectionKey,
  });

  const {
    providers,
    defaultProviderId,
    hasApiKeyConfigured,
    loading: providersLoading,
    refresh: refreshProviders,
  } = useProviders();
  // User-driven provider override. Cleared when the user picks a model
  // directly (the pill should follow the model). The effective
  // `selectedProviderId` is derived below from this override + the
  // current model's brand + the configured default — so the pill is
  // always coherent with what's actually being sent on chat_send.
  const [providerOverride, setProviderOverride] = useState<string | null>(null);

  // Effective provider id: derived from the currently selected model
  // unless the user explicitly overrode it AND the override still
  // matches the model's brand. Deriving keeps the pill in sync when
  // `useModels` swaps the model on thread open (e.g. it picks
  // `kimi-k2.6` from `agents.defaults.model`) without needing a
  // second effect to chase the model state.
  const selectedProviderId = useMemo(() => {
    const brandFromModel = selectedModel?.brand ?? selectedModel?.provider ?? null;
    if (
      providerOverride &&
      providers.some((p) => p.id === providerOverride) &&
      // Only honor the override while it's still consistent with the
      // model. If the model has a different brand (e.g. user picked a
      // different model via the model dropdown), the override is
      // stale — fall through to derive from the model.
      (!brandFromModel || brandFromModel === providerOverride)
    ) {
      return providerOverride;
    }
    if (brandFromModel && providers.some((p) => p.id === brandFromModel)) {
      return brandFromModel;
    }
    return defaultProviderId;
  }, [providerOverride, selectedModel, providers, defaultProviderId]);

  const filteredModels = useMemo(() => {
    if (!selectedProviderId) return models;
    // selectedProviderId is a *brand* (e.g. `moonshot`), not a config-
    // provider key. Match models by brand. Falls back to the full list
    // when the filter would yield nothing — defensive against models
    // that haven't been brand-tagged yet.
    const filtered = models.filter(
      (model) => (model.brand ?? model.provider) === selectedProviderId,
    );
    return filtered.length > 0 ? filtered : models;
  }, [models, selectedProviderId]);

  const handleSelectProvider = useCallback(
    (providerId: string | null) => {
      setProviderOverride(providerId);
      // Pick a sensible model for the new brand so the dropdown isn't
      // left pointing at a model that's been filtered out. Prefer one
      // marked isDefault, fall back to the first available of the same
      // brand, then to anything.
      const matching = providerId
        ? models.filter((model) => (model.brand ?? model.provider) === providerId)
        : models;
      const candidates = matching.length > 0 ? matching : models;
      const next = candidates.find((m) => m.isDefault) ?? candidates[0] ?? null;
      setSelectedModelId(next ? next.id : null);
    },
    [models, setSelectedModelId],
  );

  const {
    collaborationModes,
    selectedCollaborationMode,
    selectedCollaborationModeId,
    setSelectedCollaborationModeId,
  } = useCollaborationModes({
    activeWorkspace,
    enabled: appSettings.collaborationModesEnabled,
    preferredModeId: preferredCollabModeId,
    selectionKey: threadCodexSelectionKey,
    onDebug: addDebugEntry,
  });

  const [selectedCodexArgsOverride, setSelectedCodexArgsOverride] = useState<string | null>(null);
  useEffect(() => {
    setSelectedCodexArgsOverride(normalizeCodexArgsInput(preferredCodexArgsOverride));
  }, [preferredCodexArgsOverride]);

  const [appView, setAppView] = useState<AppView>(AppView.Calendar);
  const [selectedSessionKey, setSelectedSessionKey] = useState<string | null>(null);
  const [providerSetupOpen, setProviderSetupOpen] = useState(false);
  const { threads: chatThreads, refetch: refetchChatThreads } = useChatThreads();

  const onNewChat = useCallback(() => {
    setSelectedSessionKey(`chat:${crypto.randomUUID()}`);
    setAppView(AppView.Chat);
  }, []);

  const onSelectCalendar = useCallback(() => {
    setAppView("calendar");
  }, []);

  const onSelectThread = useCallback((sessionKey: string) => {
    setSelectedSessionKey(sessionKey);
    setAppView("chat");
  }, []);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as { sessionId?: string };
      if (detail?.sessionId) {
        onSelectThread(detail.sessionId);
      }
    };
    window.addEventListener("klynt:navigate-to-thread", handler);
    return () => window.removeEventListener("klynt:navigate-to-thread", handler);
  }, [onSelectThread]);

  const {
    handleSelectModel,
    handleSelectEffort,
    handleSelectCollaborationMode,
    handleSelectAccessMode,
    handleSelectCodexArgsOverride,
  } = useThreadSelectionHandlersOrchestration({
    appSettingsLoading,
    setAppSettings,
    queueSaveSettings,
    activeThreadIdRef,
    setSelectedModelId,
    setSelectedEffort,
    setSelectedCollaborationModeId,
    setAccessMode,
    setSelectedCodexArgsOverride,
    persistThreadCodexParams,
  });
  const commitMessageModelId = useMemo(
    () => effectiveCommitMessageModelId(filteredModels, appSettings.commitMessageModelId),
    [filteredModels, appSettings.commitMessageModelId],
  );

  const composerShortcuts = useMemo(
    () => ({
      modelShortcut: appSettings.composerModelShortcut,
      accessShortcut: appSettings.composerAccessShortcut,
      reasoningShortcut: appSettings.composerReasoningShortcut,
      collaborationShortcut: appSettings.collaborationModesEnabled
        ? appSettings.composerCollaborationShortcut
        : null,
      models: filteredModels,
      collaborationModes,
      selectedModelId,
      onSelectModel: handleSelectModel,
      selectedCollaborationModeId,
      onSelectCollaborationMode: handleSelectCollaborationMode,
      accessMode,
      onSelectAccessMode: handleSelectAccessMode,
      reasoningOptions,
      selectedEffort,
      onSelectEffort: handleSelectEffort,
      reasoningSupported,
    }),
    [
      appSettings.composerModelShortcut,
      appSettings.composerAccessShortcut,
      appSettings.composerReasoningShortcut,
      appSettings.collaborationModesEnabled,
      appSettings.composerCollaborationShortcut,
      filteredModels,
      collaborationModes,
      selectedModelId,
      handleSelectModel,
      selectedCollaborationModeId,
      handleSelectCollaborationMode,
      accessMode,
      handleSelectAccessMode,
      reasoningOptions,
      selectedEffort,
      handleSelectEffort,
      reasoningSupported,
    ],
  );

  useComposerShortcuts({
    textareaRef: composerInputRef,
    ...composerShortcuts,
  });

  useComposerShortcuts({
    textareaRef: workspaceHomeTextareaRef,
    ...composerShortcuts,
  });

  useComposerMenuActions({
    models: filteredModels,
    selectedModelId,
    onSelectModel: handleSelectModel,
    collaborationModes,
    selectedCollaborationModeId,
    onSelectCollaborationMode: handleSelectCollaborationMode,
    accessMode,
    onSelectAccessMode: handleSelectAccessMode,
    reasoningOptions,
    selectedEffort,
    onSelectEffort: handleSelectEffort,
    reasoningSupported,
    onFocusComposer: () => composerInputRef.current?.focus(),
  });
  const { skills } = useSkills(activeWorkspace);
  const {
    prompts,
    createPrompt,
    updatePrompt,
    deletePrompt,
    movePrompt,
    getWorkspacePromptsDir,
    getGlobalPromptsDir,
  } = useCustomPrompts(activeWorkspace);
  const resolvedModel = selectedModel?.model ?? null;
  const resolvedEffort = reasoningSupported ? selectedEffort : null;

  const {
    handleThreadCodexMetadataDetected,
    codexArgsOptions,
    ensureWorkspaceRuntimeCodexArgs,
    getThreadArgsBadge,
  } = useMainAppThreadCodexState({
    appCodexArgs: appSettings.codexArgs,
    selectedCodexArgsOverride,
    getThreadCodexParams,
    patchThreadCodexParams,
  });

  const { collaborationModePayload } = useCollaborationModeSelection({
    selectedCollaborationMode,
    selectedCollaborationModeId,
    selectedEffort: resolvedEffort,
    resolvedModel,
  });

  const {
    setActiveThreadId,
    hasLocalThreadSnapshot,
    activeThreadId,
    activeItems,
    approvals,
    userInputRequests,
    threadsByWorkspace,
    threadParentById,
    isSubagentThread,
    threadStatusById: assistantThreadStatusById,
    markProcessing,
    threadResumeLoadingById,
    threadListLoadingByWorkspace,
    threadListPagingByWorkspace,
    threadListCursorByWorkspace,
    activeTurnIdByThread,
    tokenUsageByThread,
    planByThread,
    lastAgentMessageByThread,
    pinnedThreadsVersion,
    interruptTurn,
    removeThread,
    pinThread,
    unpinThread,
    isThreadPinned,
    getPinTimestamp,
    renameThread,
    startThreadForWorkspace,
    listThreadsForWorkspaces,
    listThreadsForWorkspace,
    loadOlderThreadsForWorkspace,
    resetWorkspaceThreads,
    refreshThread,
    sendUserMessage,
    sendUserMessageToThread,
    startFork,
    startReview,
    startUncommittedReview,
    startResume,
    startCompact,
    startApps,
    startMcp,
    startStatus,
    reviewPrompt,
    closeReviewPrompt,
    showPresetStep,
    choosePreset,
    highlightedPresetIndex,
    setHighlightedPresetIndex,
    highlightedBranchIndex,
    setHighlightedBranchIndex,
    highlightedCommitIndex,
    setHighlightedCommitIndex,
    handleReviewPromptKeyDown,
    confirmBranch,
    selectBranch,
    selectBranchAtIndex,
    selectCommit,
    selectCommitAtIndex,
    confirmCommit,
    updateCustomInstructions,
    confirmCustom,
    handleApprovalDecision,
    handleApprovalRemember,
    handleUserInputSubmit,
  } = useThreads({
    activeWorkspace,
    onWorkspaceConnected: markWorkspaceConnected,
    onDebug: addDebugEntry,
    model: resolvedModel,
    effort: resolvedEffort,
    collaborationMode: collaborationModePayload,
    accessMode,
    ensureWorkspaceRuntimeCodexArgs,
    reviewDeliveryMode: appSettings.reviewDeliveryMode,
    steerEnabled: appSettings.steerEnabled,
    threadTitleAutogenerationEnabled: appSettings.threadTitleAutogenerationEnabled,
    chatHistoryScrollbackItems: appSettingsLoading ? null : appSettings.chatHistoryScrollbackItems,
    customPrompts: prompts,
    onMessageActivity: handleThreadMessageActivity,
    threadSortKey: threadListSortKey,
    onThreadCodexMetadataDetected: handleThreadCodexMetadataDetected,
  });
  const threadStatusById = assistantThreadStatusById;
  // Assistant-mode watchdog: if a turn is processing for >90s with no event,
  // fire and reset state so the user can retry.
  useThreadWatchdog({
    threadId: activeThreadId,
    isProcessing: Boolean(activeThreadId && threadStatusById[activeThreadId]?.isProcessing),
    onFire: useCallback(
      (threadId: string) => {
        markProcessing(threadId, false);
      },
      [markProcessing],
    ),
  });

  const { connectionState: remoteThreadConnectionState } = useRemoteThreadLiveConnection({
    backendMode: appSettings.backendMode,
    activeWorkspace,
    activeThreadId,
    activeThreadHasLocalSnapshot: hasLocalThreadSnapshot(activeThreadId),
    activeThreadIsProcessing: Boolean(
      activeThreadId && threadStatusById[activeThreadId]?.isProcessing,
    ),
    refreshThread,
    reconnectWorkspace: connectWorkspace,
  });

  const {
    updaterState,
    startUpdate,
    dismissUpdate,
    postUpdateNotice,
    dismissPostUpdateNotice,
    handleTestNotificationSound,
    handleTestSystemNotification,
  } = useUpdaterController({
    enabled: updaterEnabled,
    autoCheckOnMount: !appSettingsLoading && appSettings.automaticAppUpdateChecksEnabled,
    notificationSoundsEnabled: appSettings.notificationSoundsEnabled,
    systemNotificationsEnabled: appSettings.systemNotificationsEnabled,
    subagentSystemNotificationsEnabled: appSettings.subagentSystemNotificationsEnabled,
    isSubagentThread,
    getWorkspaceName,
    onThreadNotificationSent: (workspaceId, threadId) =>
      recordPendingThreadLinkRef.current(workspaceId, threadId),
    onDebug: addDebugEntry,
    successSoundUrl,
    errorSoundUrl,
  });
  const gitState = useMainAppGitState({
    activeWorkspace,
    activeWorkspaceId,
    activeItems,
    activeThreadId,
    activeTab,
    tabletTab,
    isCompact,
    isTablet,
    setActiveTab,
    appSettings: useMemo(
      () => ({
        preloadGitDiffs: appSettings.preloadGitDiffs,
        gitDiffIgnoreWhitespaceChanges: appSettings.gitDiffIgnoreWhitespaceChanges,
        splitChatDiffView: appSettings.splitChatDiffView,
        reviewDeliveryMode: appSettings.reviewDeliveryMode,
      }),
      [
        appSettings.preloadGitDiffs,
        appSettings.gitDiffIgnoreWhitespaceChanges,
        appSettings.splitChatDiffView,
        appSettings.reviewDeliveryMode,
      ],
    ),
    addDebugEntry,
    updateWorkspaceSettings,
    commitMessageModelId,
    connectWorkspace,
    startThreadForWorkspace,
    sendUserMessageToThread,
  });
  const {
    activeWorkspaceRef,
    activeWorkspaceIdRef,
    queueGitStatusRefresh,
    alertError,
    centerMode,
    setCenterMode,
    selectedDiffPath,
    setSelectedDiffPath,
    gitPanelMode,
    setGitPanelMode,
    gitDiffViewStyle,
    setGitDiffViewStyle,
    filePanelMode,
    selectedPullRequest,
    setSelectedPullRequest,
    selectedCommitSha,
    diffSource,
    setDiffSource,
    gitStatus,
    gitLogEntries,
    gitLogAheadEntries,
    gitLogBehindEntries,
    shouldLoadDiffs,
    activeDiffs,
    activeDiffLoading,
    activeDiffError,
    shouldLoadGitHubPanelData,
    refreshGitRemote,
    branches,
    currentBranch,
    isBranchSwitcherEnabled,
    handleCheckoutBranch,
    handleCreateGitHubRepo,
    createGitHubRepoLoading,
    handleInitGitRepo,
    initGitRepoLoading,
    isLaunchingPullRequestReview,
    pullRequestReviewActions,
    runPullRequestReview,
  } = gitState;
  queueGitStatusRefreshRef.current = queueGitStatusRefresh;
  const { isExpanded: composerEditorExpanded, toggleExpanded: toggleComposerEditorExpanded } =
    useComposerEditorState();

  const composerEditorSettings = useMemo<ComposerEditorSettings>(
    () => ({
      preset: appSettings.composerEditorPreset,
      expandFenceOnSpace: appSettings.composerFenceExpandOnSpace,
      expandFenceOnEnter: appSettings.composerFenceExpandOnEnter,
      fenceLanguageTags: appSettings.composerFenceLanguageTags,
      fenceWrapSelection: appSettings.composerFenceWrapSelection,
      autoWrapPasteMultiline: appSettings.composerFenceAutoWrapPasteMultiline,
      autoWrapPasteCodeLike: appSettings.composerFenceAutoWrapPasteCodeLike,
      continueListOnShiftEnter: appSettings.composerListContinuation,
    }),
    [
      appSettings.composerEditorPreset,
      appSettings.composerFenceExpandOnSpace,
      appSettings.composerFenceExpandOnEnter,
      appSettings.composerFenceLanguageTags,
      appSettings.composerFenceWrapSelection,
      appSettings.composerFenceAutoWrapPasteMultiline,
      appSettings.composerFenceAutoWrapPasteCodeLike,
      appSettings.composerListContinuation,
    ],
  );

  const { apps } = useApps({
    activeWorkspace,
    activeThreadId,
  });

  useThreadCodexSyncOrchestration({
    activeWorkspaceId,
    activeThreadId,
    appSettings: useMemo(
      () => ({
        defaultAccessMode: appSettings.defaultAccessMode,
        lastComposerModelId: appSettings.lastComposerModelId,
        lastComposerReasoningEffort: appSettings.lastComposerReasoningEffort,
      }),
      [
        appSettings.defaultAccessMode,
        appSettings.lastComposerModelId,
        appSettings.lastComposerReasoningEffort,
      ],
    ),
    threadCodexParamsVersion,
    getThreadCodexParams,
    patchThreadCodexParams,
    setThreadCodexSelectionKey,
    setAccessMode,
    setPreferredModelId,
    setPreferredEffort,
    setPreferredCollabModeId,
    setPreferredCodexArgsOverride,
    activeThreadIdRef,
    pendingNewThreadSeedRef,
    selectedModelId,
    resolvedEffort,
    accessMode,
    selectedCollaborationModeId,
    selectedCodexArgsOverride,
  });

  const { handleSetThreadListSortKey, handleRefreshAllWorkspaceThreads } = useThreadListActions({
    threadListSortKey,
    setThreadListSortKey,
    workspaces,
    refreshWorkspaces,
    listThreadsForWorkspaces,
    resetWorkspaceThreads,
  });

  useResponseRequiredNotificationsController({
    systemNotificationsEnabled: appSettings.systemNotificationsEnabled,
    subagentSystemNotificationsEnabled: appSettings.subagentSystemNotificationsEnabled,
    isSubagentThread,
    approvals,
    userInputRequests,
    getWorkspaceName,
    onDebug: addDebugEntry,
  });

  const {
    newAgentDraftWorkspaceId,
    startingDraftThreadWorkspaceId,
    isDraftModeForActiveWorkspace: isNewAgentDraftMode,
    startNewAgentDraft,
    clearDraftState,
    clearDraftStateIfDifferentWorkspace,
    runWithDraftStart,
  } = useNewAgentDraft({
    activeWorkspace,
    activeWorkspaceId,
    activeThreadId,
  });
  const { getThreadRows } = useThreadRows(threadParentById);

  useTrayRecentThreads({
    workspaces,
    threadsByWorkspace,
    isSubagentThread,
  });

  useAutoExitEmptyDiff({
    centerMode,
    autoExitEnabled: diffSource === "local",
    activeDiffCount: activeDiffs.length,
    activeDiffLoading,
    activeDiffError,
    activeThreadId,
    isCompact,
    setCenterMode,
    setSelectedDiffPath,
    setActiveTab,
  });

  const { handleCopyThread } = useCopyThread({
    activeItems,
    onDebug: addDebugEntry,
  });

  const {
    renamePrompt: renameWorktreePrompt,
    notice: renameWorktreeNotice,
    upstreamPrompt: renameWorktreeUpstreamPrompt,
    confirmUpstream: confirmRenameWorktreeUpstream,
    openRenamePrompt: openRenameWorktreePrompt,
    handleRenameChange: handleRenameWorktreeChange,
    handleRenameCancel: handleRenameWorktreeCancel,
    handleRenameConfirm: handleRenameWorktreeConfirm,
  } = useRenameWorktreePrompt({
    workspaces,
    activeWorkspaceId,
    renameWorktree,
    renameWorktreeUpstream,
    onRenameSuccess: (workspace) => {
      resetWorkspaceThreads(workspace.id);
      void listThreadsForWorkspace(workspace);
      if (activeThreadId && activeWorkspaceId === workspace.id) {
        void refreshThread(workspace.id, activeThreadId);
      }
    },
  });

  const handleOpenRenameWorktree = useCallback(() => {
    if (activeWorkspace) {
      openRenameWorktreePrompt(activeWorkspace.id);
    }
  }, [activeWorkspace, openRenameWorktreePrompt]);

  const {
    terminalTabs,
    activeTerminalId,
    onSelectTerminal,
    onNewTerminal,
    onCloseTerminal,
    terminalState,
    ensureTerminalWithTitle,
    restartTerminalSession,
    requestTerminalFocus,
  } = useTerminalController({
    activeWorkspaceId,
    activeWorkspace,
    terminalOpen,
    onCloseTerminalPanel: closeTerminalPanel,
    onDebug: addDebugEntry,
  });

  const ensureLaunchTerminal = useCallback(
    (workspaceId: string) => ensureTerminalWithTitle(workspaceId, "launch", "Launch"),
    [ensureTerminalWithTitle],
  );

  const openTerminalWithFocus = useCallback(() => {
    if (!activeWorkspaceId) {
      return;
    }
    requestTerminalFocus();
    openTerminal();
  }, [activeWorkspaceId, openTerminal, requestTerminalFocus]);

  const handleToggleTerminalWithFocus = useCallback(() => {
    if (!activeWorkspaceId) {
      return;
    }
    if (!terminalOpen) {
      requestTerminalFocus();
    }
    handleToggleTerminal();
  }, [activeWorkspaceId, handleToggleTerminal, requestTerminalFocus, terminalOpen]);

  const launchScriptState = useWorkspaceLaunchScript({
    activeWorkspace,
    updateWorkspaceSettings,
    openTerminal: openTerminalWithFocus,
    ensureLaunchTerminal,
    restartLaunchSession: restartTerminalSession,
    terminalState,
    activeTerminalId,
  });

  const launchScriptsState = useWorkspaceLaunchScripts({
    activeWorkspace,
    updateWorkspaceSettings,
    openTerminal: openTerminalWithFocus,
    ensureLaunchTerminal: (workspaceId, entry, title) => {
      const label = entry.label?.trim() || entry.icon;
      return ensureTerminalWithTitle(workspaceId, `launch:${entry.id}`, title || `Launch ${label}`);
    },
    restartLaunchSession: restartTerminalSession,
    terminalState,
    activeTerminalId,
  });

  const worktreeSetupScriptState = useWorktreeSetupScript({
    ensureTerminalWithTitle,
    restartTerminalSession,
    openTerminal,
    onDebug: addDebugEntry,
  });

  const handleWorktreeCreated = useCallback(
    async (worktree: WorkspaceInfo, _parentWorkspace?: WorkspaceInfo) => {
      await worktreeSetupScriptState.maybeRunWorktreeSetupScript(worktree);
    },
    [worktreeSetupScriptState],
  );

  const { exitDiffView, selectWorkspace, selectHome } = useWorkspaceSelection({
    workspaces,
    isCompact,
    activeWorkspaceId,
    setActiveTab,
    setActiveWorkspaceId,
    updateWorkspaceSettings,
    setCenterMode,
    setSelectedDiffPath,
  });

  const resolveCloneProjectContext = useCallback(
    (workspace: WorkspaceInfo) => {
      const groupId = workspace.settings.groupId ?? null;
      const group = groupId
        ? appSettings.workspaceGroups.find((entry) => entry.id === groupId)
        : null;
      return {
        groupId,
        copiesFolder: group?.copiesFolder ?? null,
      };
    },
    [appSettings.workspaceGroups],
  );

  const { handleMoveWorkspace } = useWorkspaceOrderingOrchestration({
    workspaces,
    workspacesById,
    updateWorkspaceSettings,
  });

  const {
    handleSelectOpenAppId,
    handleToggleAutomaticAppUpdateChecks,
    persistProjectCopiesFolder,
  } = useMainAppSettingsActions({
    appSettings,
    setAppSettings,
    queueSaveSettings,
  });

  const openAppIconById = useOpenAppIcons(appSettings.openAppTargets);

  const {
    workspaceFromUrlPrompt,
    openWorkspaceFromUrlPrompt,
    closeWorkspaceFromUrlPrompt,
    chooseWorkspaceFromUrlDestinationPath,
    submitWorkspaceFromUrlPrompt,
    updateWorkspaceFromUrlUrl,
    updateWorkspaceFromUrlTargetFolderName,
    clearWorkspaceFromUrlDestinationPath,
    canSubmitWorkspaceFromUrlPrompt,
  } = useWorkspaceFromUrlPrompt({
    onSubmit: async (url, destinationPath, targetFolderName) => {
      await handleAddWorkspaceFromGitUrl(url, destinationPath, targetFolderName);
    },
  });

  const { appModalsProps, modalActions } = useMainAppModals({
    settingsViewComponent: showNewSettings ? SettingsShellModal : SettingsView,
    workspaces,
    workspaceGroups,
    groupedWorkspaces,
    ungroupedLabel,
    activeWorkspace,
    setActiveWorkspaceId,
    branches,
    currentBranch,
    threadRename: {
      threadsByWorkspace,
      renameThread,
    },
    git: {
      checkoutBranch: handleCheckoutBranch,
      initGitRepo: handleInitGitRepo,
      createGitHubRepo: handleCreateGitHubRepo,
      refreshGitRemote,
      initGitRepoLoading,
      createGitHubRepoLoading,
    },
    workspacePrompts: {
      addWorktreeAgent,
      addCloneAgent,
      connectWorkspace,
      updateWorkspaceSettings,
      selectWorkspace,
      handleWorktreeCreated,
      resolveCloneProjectContext,
      persistProjectCopiesFolder,
      onCompactActivate: isCompact ? () => setActiveTab("codex") : undefined,
      onWorkspacePromptError: (message, kind) => {
        addDebugEntry({
          id: `${Date.now()}-client-add-${kind}-error`,
          timestamp: Date.now(),
          source: "error",
          label: `${kind}/add error`,
          payload: message,
        });
      },
      mobileRemoteWorkspacePathPrompt,
      updateMobileRemoteWorkspacePathInput,
      appendMobileRemoteWorkspacePathFromRecent,
      cancelMobileRemoteWorkspacePathPrompt,
      submitMobileRemoteWorkspacePathPrompt,
      openWorkspaceFromUrlPrompt,
      workspaceFromUrl: {
        workspaceFromUrlPrompt,
        workspaceFromUrlCanSubmit: canSubmitWorkspaceFromUrlPrompt,
        onWorkspaceFromUrlPromptUrlChange: updateWorkspaceFromUrlUrl,
        onWorkspaceFromUrlPromptTargetFolderNameChange: updateWorkspaceFromUrlTargetFolderName,
        onWorkspaceFromUrlPromptChooseDestinationPath: chooseWorkspaceFromUrlDestinationPath,
        onWorkspaceFromUrlPromptClearDestinationPath: clearWorkspaceFromUrlDestinationPath,
        onWorkspaceFromUrlPromptCancel: closeWorkspaceFromUrlPrompt,
        onWorkspaceFromUrlPromptConfirm: submitWorkspaceFromUrlPrompt,
      },
    },
    settings: {
      handleMoveWorkspace,
      removeWorkspace,
      createWorkspaceGroup,
      renameWorkspaceGroup,
      moveWorkspaceGroup,
      deleteWorkspaceGroup,
      assignWorkspaceGroup,
      reduceTransparency,
      setReduceTransparency,
      appSettings,
      openAppIconById,
      queueSaveSettings,
      handleToggleAutomaticAppUpdateChecks,
      doctor,
      codexUpdate,
      updateWorkspaceSettings,
      scaleShortcutTitle,
      scaleShortcutText,
      handleTestNotificationSound,
      handleTestSystemNotification,
      handleMobileConnectSuccess,
      dictationModel,
    },
  });

  useBranchSwitcherShortcut({
    shortcut: appSettings.branchSwitcherShortcut,
    isEnabled: isBranchSwitcherEnabled,
    onTrigger: modalActions.openBranchSwitcher,
  });

  const handleRenameThread = useCallback(
    (workspaceId: string, threadId: string) => {
      modalActions.openRenamePrompt(workspaceId, threadId);
    },
    [modalActions],
  );

  const showHome = !activeWorkspace;
  const {
    latestAgentRuns,
    isLoadingLatestAgents,
    usageMetric,
    setUsageMetric,
    usageWorkspaceId,
    setUsageWorkspaceId,
    usageWorkspaceOptions,
    localUsageSnapshot,
    isLoadingLocalUsage,
    localUsageError,
    refreshLocalUsage,
  } = useWorkspaceInsightsOrchestration({
    workspaces,
    workspacesById,
    hasLoaded,
    showHome,
    threadsByWorkspace,
    lastAgentMessageByThread,
    threadStatusById,
    threadListLoadingByWorkspace,
    getWorkspaceGroupName,
  });

  const activeTokenUsage = activeThreadId ? (tokenUsageByThread[activeThreadId] ?? null) : null;
  const activePlan = activeThreadId ? (planByThread[activeThreadId] ?? null) : null;
  const hasActivePlan = Boolean(
    activePlan && (activePlan.steps.length > 0 || activePlan.explanation),
  );
  const composerWorkspaceState = useMainAppComposerWorkspaceState({
    view: {
      activeTab,
      tabletTab,
      centerMode,
      isCompact,
      isTablet,
      rightPanelCollapsed,
      filePanelMode,
    },
    workspace: {
      activeWorkspace,
      activeWorkspaceId,
      isNewAgentDraftMode,
      startingDraftThreadWorkspaceId,
      threadsByWorkspace,
    },
    thread: {
      activeThreadId,
      activeItems,
      threadStatusById,
      activeTurnIdByThread,
      userInputRequests,
    },
    settings: {
      steerEnabled: appSettings.steerEnabled,
      followUpMessageBehavior: appSettings.followUpMessageBehavior,
      experimentalAppsEnabled: appSettings.experimentalAppsEnabled,
      pauseQueuedMessagesWhenResponseRequired: appSettings.pauseQueuedMessagesWhenResponseRequired,
    },
    models: {
      models: filteredModels,
      selectedModelId,
      resolvedEffort,
      collaborationModePayload,
    },
    refs: {
      composerInputRef,
      workspaceHomeTextareaRef,
    },
    actions: {
      connectWorkspace,
      startThreadForWorkspace,
      sendUserMessage,
      sendUserMessageToThread,
      seedThreadCodexParams: patchThreadCodexParams,
      startFork,
      startReview,
      startResume,
      startCompact,
      startApps,
      startMcp,
      startStatus,
      addWorktreeAgent,
      handleWorktreeCreated,
      addDebugEntry,
    },
  });
  const {
    files,
    setFileAutocompleteActive,
    showWorkspaceHome,
    showComposer,
    canInterrupt,
    recentThreadInstances,
    recentThreadsUpdatedAt,
    clearActiveImages,
    removeImagesForThread,
    handleSend,
    setPrefillDraft,
    clearDraftForThread,
    workspaceHomeState,
    agentMdState,
  } = composerWorkspaceState;
  const {
    runs: workspaceRuns,
    draft: workspacePrompt,
    runMode: workspaceRunMode,
    modelSelections: workspaceModelSelections,
    error: workspaceRunError,
    isSubmitting: workspaceRunSubmitting,
    setDraft: setWorkspacePrompt,
    setRunMode: setWorkspaceRunMode,
    toggleModelSelection: toggleWorkspaceModelSelection,
    setModelCount: setWorkspaceModelCount,
    startRun: startWorkspaceRun,
  } = workspaceHomeState;
  const {
    content: agentMdContent,
    exists: agentMdExists,
    truncated: agentMdTruncated,
    isLoading: agentMdLoading,
    isSaving: agentMdSaving,
    error: agentMdError,
    isDirty: agentMdDirty,
    setContent: setAgentMdContent,
    refresh: refreshAgentMd,
    save: saveAgentMd,
  } = agentMdState;
  const promptActions = useMainAppPromptActions({
    activeWorkspace,
    connectWorkspace,
    startThreadForWorkspace,
    sendUserMessageToThread,
    alertError,
    createPrompt,
    updatePrompt,
    deletePrompt,
    movePrompt,
    getWorkspacePromptsDir,
    getGlobalPromptsDir,
  });
  const worktreeState = useMainAppWorktreeState({
    activeWorkspace,
    workspacesById,
    renameWorktreePrompt,
    renameWorktreeNotice,
    renameWorktreeUpstreamPrompt,
    confirmRenameWorktreeUpstream,
    handleOpenRenameWorktree,
    handleRenameWorktreeChange,
    handleRenameWorktreeCancel,
    handleRenameWorktreeConfirm,
  });
  const { baseWorkspaceRef } = worktreeState;

  useMainAppWorkspaceLifecycle({
    activeTab,
    isTablet,
    setActiveTab,
    workspaces,
    hasLoaded,
    connectWorkspace,
    listThreadsForWorkspaces,
    refreshWorkspaces,
    backendMode: appSettings.backendMode,
    activeWorkspace,
    activeThreadId,
    threadStatusById,
    remoteThreadConnectionState,
    refreshThread,
  });

  const {
    handleAddWorkspace,
    handleAddWorkspaceFromGitUrl,
    handleAddAgent,
    handleAddWorktreeAgent,
    handleAddCloneAgent,
    dropTargetRef: workspaceDropTargetRef,
    isDragOver: isWorkspaceDropActive,
    handleDragOver: handleWorkspaceDragOver,
    handleDragEnter: handleWorkspaceDragEnter,
    handleDragLeave: handleWorkspaceDragLeave,
    handleDrop: handleWorkspaceDrop,
  } = useMainAppWorkspaceActions({
    workspaceActions: {
      isCompact,
      addWorkspace,
      addWorkspaceFromPath,
      addWorkspaceFromGitUrl,
      addWorkspacesFromPaths,
      setActiveThreadId,
      setActiveTab,
      exitDiffView,
      selectWorkspace,
      onStartNewAgentDraft: startNewAgentDraft,
      openWorktreePrompt: modalActions.openWorktreePrompt,
      openClonePrompt: modalActions.openClonePrompt,
      composerInputRef,
      onDebug: addDebugEntry,
    },
  });

  useInterruptShortcut({
    isEnabled: canInterrupt,
    shortcut: appSettings.interruptShortcut,
    onTrigger: () => {
      void interruptTurn();
    },
  });

  const selectedCommitEntry = useMemo(() => {
    if (!selectedCommitSha) {
      return null;
    }
    return (
      [...gitLogAheadEntries, ...gitLogBehindEntries, ...gitLogEntries].find(
        (entry) => entry.sha === selectedCommitSha,
      ) ?? null
    );
  }, [gitLogAheadEntries, gitLogBehindEntries, gitLogEntries, selectedCommitSha]);

  const {
    handleSelectPullRequest,
    resetPullRequestSelection,
    composerContextActions,
    composerSendLabel,
    handleComposerSend,
  } = usePullRequestComposer({
    activeWorkspace,
    selectedPullRequest,
    selectedCommit: selectedCommitEntry,
    filePanelMode,
    gitPanelMode,
    centerMode,
    isCompact,
    setSelectedPullRequest,
    setDiffSource,
    setSelectedDiffPath,
    setCenterMode,
    setGitPanelMode,
    setPrefillDraft,
    setActiveTab,
    pullRequestReviewActions,
    pullRequestReviewLaunching: isLaunchingPullRequestReview,
    runPullRequestReview,
    startReview,
    clearActiveImages,
    handleSend,
  });

  const {
    handleComposerSendWithDraftStart,
    handleSelectWorkspaceInstance,
    handleOpenThreadLink,
    handleArchiveActiveThread,
  } = useThreadUiOrchestration({
    activeWorkspaceId,
    activeThreadId,
    accessMode,
    selectedCollaborationModeId,
    selectedCodexArgsOverride,
    pendingNewThreadSeedRef,
    runWithDraftStart,
    handleComposerSend,
    clearDraftState,
    exitDiffView,
    resetPullRequestSelection,
    selectWorkspace,
    setActiveThreadId,
    setActiveTab,
    isCompact,
    removeThread,
    clearDraftForThread,
    removeImagesForThread,
  });

  const handleOpenThreadLinkFromExternal = useCallback(
    (workspaceId: string, threadId: string) => {
      handleOpenThreadLink(threadId, workspaceId);
    },
    [handleOpenThreadLink],
  );

  const { recordPendingThreadLink, openThreadLinkOrQueue } = useSystemNotificationThreadLinks({
    hasLoadedWorkspaces: hasLoaded,
    workspacesById,
    refreshWorkspaces,
    connectWorkspace,
    openThreadLink: handleOpenThreadLinkFromExternal,
  });

  useTauriEvent(
    subscribeTrayOpenThread,
    ({ workspaceId, threadId }: { workspaceId: string; threadId: string }) => {
      openThreadLinkOrQueue(workspaceId, threadId);
    },
  );

  useEffect(() => {
    recordPendingThreadLinkRef.current = recordPendingThreadLink;
    return () => {
      recordPendingThreadLinkRef.current = () => {};
    };
  }, [recordPendingThreadLink]);

  const { handlePlanAccept, handlePlanSubmitChanges } = usePlanReadyActions({
    activeWorkspace,
    activeThreadId,
    collaborationModes,
    resolvedModel,
    resolvedEffort,
    connectWorkspace,
    sendUserMessageToThread,
    setSelectedCollaborationModeId,
    persistThreadCodexParams,
  });

  const { isThreadOpen, dropOverlayActive, dropOverlayText, appClassName, appStyle } =
    useAppShellOrchestration({
      sidebarCollapsed,
      rightPanelCollapsed,
      shouldReduceTransparency,
      isWorkspaceDropActive,
      centerMode,
      appView,
      selectedDiffPath,
      showComposer,
      activeThreadId,
      sidebarWidth,
      chatDiffSplitPositionPercent,
      rightPanelWidth,
      planPanelHeight,
      terminalPanelHeight,
      debugPanelHeight,
      appSettings,
    });

  const sidebarMenuOrchestration = useMainAppSidebarMenuOrchestration({
    sidebarActions: {
      openSettings: modalActions.openSettings,
      resetPullRequestSelection,
      clearDraftState,
      clearDraftStateIfDifferentWorkspace,
      selectHome,
      exitDiffView,
      selectWorkspace,
      setActiveThreadId,
      connectWorkspace,
      isCompact,
      setActiveTab,
      workspacesById,
      updateWorkspaceSettings,
      removeThread,
      clearDraftForThread,
      removeImagesForThread,
      refreshThread,
      handleRenameThread,
      removeWorkspace,
      removeWorktree,
      loadOlderThreadsForWorkspace,
      listThreadsForWorkspace,
    },
    workspaceCycling: {
      workspaces,
      groupedWorkspaces,
      threadsByWorkspace,
      getThreadRows,
      getPinTimestamp,
      pinnedThreadsVersion,
      activeWorkspaceIdRef,
      activeThreadIdRef,
      exitDiffView,
      resetPullRequestSelection,
      selectWorkspace,
      setActiveThreadId,
    },
    appMenu: {
      activeWorkspaceRef,
      baseWorkspaceRef,
      onAddWorkspace: handleAddWorkspace,
      onAddWorkspaceFromUrl: openWorkspaceFromUrlPrompt,
      onAddAgent: handleAddAgent,
      onAddWorktreeAgent: handleAddWorktreeAgent,
      onAddCloneAgent: handleAddCloneAgent,
      onToggleDebug: handleDebugClick,
      onToggleTerminal: handleToggleTerminalWithFocus,
      sidebarCollapsed,
      rightPanelCollapsed,
      onExpandSidebar: expandSidebar,
      onCollapseSidebar: collapseSidebar,
      onExpandRightPanel: expandRightPanel,
      onCollapseRightPanel: collapseRightPanel,
    },
    appSettings,
    onDebug: addDebugEntry,
  });
  useArchiveShortcut({
    isEnabled: isThreadOpen,
    shortcut: appSettings.archiveThreadShortcut,
    onTrigger: handleArchiveActiveThread,
  });
  const gitRootOverride = activeWorkspace?.settings.gitRoot;
  const hasGitRootOverride =
    typeof gitRootOverride === "string" && gitRootOverride.trim().length > 0;
  const showGitInitBanner =
    Boolean(activeWorkspace) && !hasGitRootOverride && isMissingRepo(gitStatus.error);
  const displayNodes = useMainAppDisplayNodes({
    centerMode,
    gitDiffViewStyle,
    setGitDiffViewStyle,
    isCompact,
    rightPanelCollapsed,
    sidebarToggleProps,
    workspaceHomeProps: activeWorkspace
      ? {
          workspace: activeWorkspace,
          showGitInitBanner,
          initGitRepoLoading,
          onInitGitRepo: modalActions.openInitGitRepoPrompt,
          runs: workspaceRuns,
          recentThreadInstances,
          recentThreadsUpdatedAt,
          prompt: workspacePrompt,
          onPromptChange: setWorkspacePrompt,
          onStartRun: startWorkspaceRun,
          runMode: workspaceRunMode,
          onRunModeChange: setWorkspaceRunMode,
          models: filteredModels,
          selectedModelId,
          onSelectModel: setSelectedModelId,
          modelSelections: workspaceModelSelections,
          onToggleModel: toggleWorkspaceModelSelection,
          onModelCountChange: setWorkspaceModelCount,
          collaborationModes,
          selectedCollaborationModeId,
          onSelectCollaborationMode: setSelectedCollaborationModeId,
          reasoningOptions,
          selectedEffort,
          onSelectEffort: setSelectedEffort,
          reasoningSupported,
          error: workspaceRunError,
          isSubmitting: workspaceRunSubmitting,
          activeWorkspaceId,
          activeThreadId,
          threadStatusById,
          onSelectInstance: handleSelectWorkspaceInstance,
          skills,
          appsEnabled: appSettings.experimentalAppsEnabled,
          apps,
          prompts,
          files,
          onFileAutocompleteActiveChange: setFileAutocompleteActive,
          dictationEnabled: appSettings.dictationEnabled && dictationReady,
          dictationState,
          dictationLevel,
          onToggleDictation: handleToggleDictation,
          onCancelDictation: cancelDictation,
          onOpenDictationSettings: () => modalActions.openSettings("dictation"),
          dictationError,
          onDismissDictationError: clearDictationError,
          dictationHint,
          onDismissDictationHint: clearDictationHint,
          dictationTranscript,
          onDictationTranscriptHandled: clearDictationTranscript,
          textareaRef: workspaceHomeTextareaRef,
          agentMdContent,
          agentMdExists,
          agentMdTruncated,
          agentMdLoading,
          agentMdSaving,
          agentMdError,
          agentMdDirty,
          onAgentMdChange: setAgentMdContent,
          onAgentMdRefresh: () => {
            void refreshAgentMd();
          },
          onAgentMdSave: () => {
            void saveAgentMd();
          },
        }
      : null,
  });
  const { workspaceHomeNode } = displayNodes;
  const layoutSurfaces = useMainAppLayoutSurfaces({
    appSettings: useMemo(
      () => ({
        composerCodeBlockCopyUseModifier: appSettings.composerCodeBlockCopyUseModifier,
        showMessageFilePath: appSettings.showMessageFilePath,
        openAppTargets: appSettings.openAppTargets,
        selectedOpenAppId: appSettings.selectedOpenAppId,
        experimentalAppsEnabled: appSettings.experimentalAppsEnabled,
        followUpMessageBehavior: appSettings.followUpMessageBehavior,
        composerFollowUpHintEnabled: appSettings.composerFollowUpHintEnabled,
        dictationEnabled: appSettings.dictationEnabled,
        splitChatDiffView: appSettings.splitChatDiffView,
        gitDiffIgnoreWhitespaceChanges: appSettings.gitDiffIgnoreWhitespaceChanges,
      }),
      [
        appSettings.composerCodeBlockCopyUseModifier,
        appSettings.showMessageFilePath,
        appSettings.openAppTargets,
        appSettings.selectedOpenAppId,
        appSettings.experimentalAppsEnabled,
        appSettings.followUpMessageBehavior,
        appSettings.composerFollowUpHintEnabled,
        appSettings.dictationEnabled,
        appSettings.splitChatDiffView,
        appSettings.gitDiffIgnoreWhitespaceChanges,
      ],
    ),
    workspaces,
    groupedWorkspaces,
    workspaceGroupsCount: workspaceGroups.length,
    deletingWorktreeIds,
    newAgentDraftWorkspaceId,
    startingDraftThreadWorkspaceId,
    threadsByWorkspace,
    threadParentById,
    threadStatusById,
    threadResumeLoadingById,
    threadListLoadingByWorkspace,
    threadListPagingByWorkspace,
    threadListCursorByWorkspace,
    pinnedThreadsVersion,
    threadListSortKey,
    onSetThreadListSortKey: handleSetThreadListSortKey,
    threadListOrganizeMode,
    onSetThreadListOrganizeMode: setThreadListOrganizeMode,
    onRefreshAllThreads: handleRefreshAllWorkspaceThreads,
    activeWorkspace,
    activeWorkspaceId,
    activeThreadId,
    activeItems,
    userInputRequests,
    approvals,
    onDecision: handleApprovalDecision,
    onRemember: handleApprovalRemember,
    onUserInputSubmit: handleUserInputSubmit,
    onPlanAccept: handlePlanAccept,
    onPlanSubmitChanges: handlePlanSubmitChanges,
    activePlan,
    activeTokenUsage,
    latestAgentRuns,
    isLoadingLatestAgents,
    localUsageSnapshot,
    isLoadingLocalUsage,
    localUsageError,
    onRefreshLocalUsage: () => {
      refreshLocalUsage()?.catch(() => {});
    },
    usageMetric,
    onUsageMetricChange: setUsageMetric,
    usageWorkspaceId,
    usageWorkspaceOptions,
    onUsageWorkspaceChange: setUsageWorkspaceId,
    gitState,
    composerWorkspaceState,
    promptActions,
    worktreeState,
    sidebarHandlers: sidebarMenuOrchestration,
    displayNodes,
    threadPinning: {
      pinThread,
      unpinThread,
      isThreadPinned,
      getPinTimestamp,
      getThreadArgsBadge,
    },
    workspaceDrop: {
      workspaceDropTargetRef,
      isWorkspaceDropActive: dropOverlayActive,
      workspaceDropText: dropOverlayText,
      onWorkspaceDragOver: handleWorkspaceDragOver,
      onWorkspaceDragEnter: handleWorkspaceDragEnter,
      onWorkspaceDragLeave: handleWorkspaceDragLeave,
      onWorkspaceDrop: handleWorkspaceDrop,
    },
    threadNavigation: {
      exitDiffView,
      clearDraftState,
      selectWorkspace,
      setActiveThreadId,
      resetPullRequestSelection,
      selectHome,
    },
    pullRequestComposer: {
      composerSendLabel,
      handleSelectPullRequest,
    },
    dictationUi: {
      onOpenDictationSettings: () => modalActions.openSettings("dictation"),
      dictationTranscript,
      dictationError,
      dictationHint,
    },
    openAppIconById,
    openInitGitRepoPrompt: modalActions.openInitGitRepoPrompt,
    startUncommittedReview,
    handleAddWorkspace,
    openWorkspaceFromUrlPrompt,
    handleAddAgent,
    handleAddWorktreeAgent,
    handleAddCloneAgent,
    handleOpenThreadLink,
    handleSelectOpenAppId,
    handleCopyThread,
    handleToggleTerminalWithFocus,
    launchScriptState,
    launchScriptsState,
    models: filteredModels,
    selectedModelId,
    onSelectModel: handleSelectModel,
    providers,
    selectedProviderId,
    onSelectProvider: handleSelectProvider,
    collaborationModes,
    selectedCollaborationModeId,
    onSelectCollaborationMode: handleSelectCollaborationMode,
    reasoningOptions,
    selectedEffort,
    onSelectEffort: handleSelectEffort,
    reasoningSupported,
    codexArgsOptions,
    selectedCodexArgsOverride,
    onSelectCodexArgsOverride: handleSelectCodexArgsOverride,
    accessMode,
    onSelectAccessMode: handleSelectAccessMode,
    skills,
    apps,
    prompts,
    composerInputRef,
    composerEditorSettings,
    composerEditorExpanded,
    onToggleComposerEditorExpanded: toggleComposerEditorExpanded,
    dictationReady,
    dictationState,
    dictationLevel,
    onToggleDictation: handleToggleDictation,
    onCancelDictation: cancelDictation,
    clearDictationTranscript,
    clearDictationError,
    clearDictationHint,
    composerContextActions,
    reviewPrompt,
    closeReviewPrompt,
    showPresetStep,
    choosePreset,
    highlightedPresetIndex,
    setHighlightedPresetIndex,
    highlightedBranchIndex,
    setHighlightedBranchIndex,
    highlightedCommitIndex,
    setHighlightedCommitIndex,
    handleReviewPromptKeyDown,
    selectBranch,
    selectBranchAtIndex,
    confirmBranch,
    selectCommit,
    selectCommitAtIndex,
    confirmCommit,
    updateCustomInstructions,
    confirmCustom,
    handleComposerSendWithDraftStart,
    interruptTurn,
    terminalOpen,
    debugOpen,
    debugEntries,
    terminalTabs,
    activeTerminalId,
    onSelectTerminal,
    onNewTerminal,
    onCloseTerminal,
    terminalState,
    onClearDebug: clearDebugEntries,
    onCopyDebug: handleCopyDebug,
    onResizeDebug: onDebugPanelResizeStart,
    onResizeTerminal: onTerminalPanelResizeStart,
    isCompact,
    appModalsAboutOpen: appModalsProps.settingsOpen && appModalsProps.settingsSection === "about",
    updaterState,
    startUpdate,
    dismissUpdate,
    postUpdateNotice,
    dismissPostUpdateNotice,
    errorToasts,
    dismissErrorToast,
    showDebugButton,
    handleDebugClick,
    chatView: {
      appView,
      selectedSessionKey,
      onNewChat,
      onSelectThread,
      onSelectCalendar,
      activeNavId: appView === "calendar" ? "calendar" : null,
      chatThreads,
      refetchChatThreads,
    },
  });

  const klyntbotSurface = useKlyntbotSurfaceProps(appView === "chat" ? selectedSessionKey : null);

  const status = activeThreadId ? threadStatusById[activeThreadId] : null;
  const { isStuck, stuckDurationMs } = useStuckThreadDetector(
    status?.isProcessing ?? false,
    status?.processingStartedAt ?? null,
  );

  const finalLayoutSurfaces = klyntbotSurface
    ? {
        ...layoutSurfaces,
        primary: {
          ...layoutSurfaces.primary,
          messagesProps: {
            ...layoutSurfaces.primary.messagesProps,
            ...klyntbotSurface.messagesProps,
          },
          composerProps: layoutSurfaces.primary.composerProps
            ? {
                ...layoutSurfaces.primary.composerProps,
                ...klyntbotSurface.composerProps,
                isStuck,
              }
            : layoutSurfaces.primary.composerProps,
        },
      }
    : layoutSurfaces;

  const {
    sidebarNode,
    messagesNode,
    composerNode,
    approvalToastsNode,
    updateToastNode,
    errorToastsNode,
    homeNode,
    desktopTopbarLeftNode,
    gitDiffPanelNode,
    gitDiffViewerNode,
    planPanelNode,
    debugPanelNode,
    terminalDockNode,
  } = useMainAppLayoutNodes(finalLayoutSurfaces);

  const chatMessagesNode = klyntbotSurface ? (
    <>
      <ChatErrorBanner error={klyntbotSurface.error} onDismiss={klyntbotSurface.onDismissError} />
      {activeThreadId && isStuck && (
        <StuckThreadBanner
          durationMs={stuckDurationMs}
          onReset={() => {
            markProcessing(activeThreadId, false);
          }}
        />
      )}
      {messagesNode}
    </>
  ) : (
    <>
      {activeThreadId && isStuck && (
        <StuckThreadBanner
          durationMs={stuckDurationMs}
          onReset={() => {
            markProcessing(activeThreadId, false);
          }}
        />
      )}
      {messagesNode}
    </>
  );
  // Coding mode reuses the polished Messages UI by piping the new
  // `agent:thread_event` stream through CodingThreadView's adapter. Same
  // bubbles, markdown, code blocks, copy buttons as assistant mode — only
  const mainMessagesNode =
    showWorkspaceHome && appView !== "chat" ? workspaceHomeNode : chatMessagesNode;

  const showProviderSetup = !providersLoading && !hasApiKeyConfigured && appView === AppView.Chat;

  const compactThreadConnectionState: "live" | "polling" | "disconnected" =
    !activeWorkspace?.connected ? "disconnected" : remoteThreadConnectionState;
  const mainAppShellProps = useMainAppShellProps({
    shell: {
      appClassName,
      isResizing,
      appStyle,
      appRef,
      sidebarToggleProps,
      shouldLoadGitHubPanelData,
      appModalsProps,
      showMobileSetupWizard,
      mobileSetupWizardProps,
    },
    gitHubPanelDataProps: {
      activeWorkspace,
      gitPanelMode,
      shouldLoadDiffs,
      diffSource,
      selectedPullRequestNumber: selectedPullRequest?.number ?? null,
    },
    appLayout: {
      showHome: showHome && appView !== "chat" && appView !== "calendar",
      centerMode: (() => {
        switch (appView) {
          case "chat":
            return "chat";
          case "calendar":
            return "calendar";
          default:
            return centerMode;
        }
      })(),
      preloadGitDiffs: appSettings.preloadGitDiffs,
      splitChatDiffView: appSettings.splitChatDiffView,
      hasActivePlan: hasActivePlan,
      activeWorkspace: (Boolean(activeWorkspace) || appView === "chat") && appView !== "calendar",
      sidebarNode,
      messagesNode: showProviderSetup ? (
        <ProviderSetupPrompt onOpenSettings={() => setProviderSetupOpen(true)} />
      ) : (
        mainMessagesNode
      ),
      composerNode: showProviderSetup ? null : composerNode,
      approvalToastsNode,
      updateToastNode,
      errorToastsNode,
      homeNode,
      dashboardNode: appView === "calendar" ? <Dashboard /> : null,
      gitDiffPanelNode,
      gitDiffViewerNode,
      planPanelNode,
      debugPanelNode,
      terminalDockNode,
      onSidebarResizeStart,
      onChatDiffSplitPositionResizeStart,
      onRightPanelResizeStart,
      onPlanPanelResizeStart,
    },
    topbar: {
      isCompact,
      desktopTopbarLeftNode,
      hasActiveWorkspace: Boolean(activeWorkspace),
      backendMode: appSettings.backendMode,
      remoteThreadConnectionState: compactThreadConnectionState,
    },
  });

  return (
    <>
      <MainAppShell {...mainAppShellProps} />
      {providerSetupOpen && (
        <ProviderSetupModal
          onClose={() => {
            setProviderSetupOpen(false);
            void refreshProviders();
            void refreshModels();
          }}
        />
      )}
    </>
  );
}
