import {
  tailscaleDaemonCommandPreview as fetchTailscaleDaemonCommandPreview,
  tailscaleStatus as fetchTailscaleStatus,
  listWorkspaces,
  tailscaleDaemonStart,
  tailscaleDaemonStatus,
  tailscaleDaemonStop,
} from "@services/tauri";
import { DEFAULT_REMOTE_HOST } from "@settings/components/settingsViewConstants";
import { isMobilePlatform } from "@utils/platformPaths";
import type { Dispatch, SetStateAction } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import type {
  AppSettings,
  TailscaleDaemonCommandPreview,
  TailscaleStatus,
  TcpDaemonStatus,
} from "@/types";

type UseSettingsServerSectionArgs = {
  appSettings: AppSettings;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  onMobileConnectSuccess?: () => Promise<void> | void;
};

export type AddRemoteBackendDraft = {
  name: string;
  host: string;
  token: string;
};

export type SettingsServerSectionProps = {
  appSettings: AppSettings;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  isMobilePlatform: boolean;
  mobileConnectBusy: boolean;
  mobileConnectStatusText: string | null;
  mobileConnectStatusError: boolean;
  remoteBackends: AppSettings["remoteBackends"];
  activeRemoteBackendId: string | null;
  remoteStatusText: string | null;
  remoteStatusError: boolean;
  remoteNameError: string | null;
  remoteHostError: string | null;
  remoteNameDraft: string;
  remoteHostDraft: string;
  remoteTokenDraft: string;
  nextRemoteNameSuggestion: string;
  tailscaleStatus: TailscaleStatus | null;
  tailscaleStatusBusy: boolean;
  tailscaleStatusError: string | null;
  tailscaleCommandPreview: TailscaleDaemonCommandPreview | null;
  tailscaleCommandBusy: boolean;
  tailscaleCommandError: string | null;
  tcpDaemonStatus: TcpDaemonStatus | null;
  tcpDaemonBusyAction: "start" | "stop" | "status" | null;
  onSetRemoteNameDraft: Dispatch<SetStateAction<string>>;
  onSetRemoteHostDraft: Dispatch<SetStateAction<string>>;
  onSetRemoteTokenDraft: Dispatch<SetStateAction<string>>;
  onCommitRemoteName: () => Promise<void>;
  onCommitRemoteHost: () => Promise<void>;
  onCommitRemoteToken: () => Promise<void>;
  onSelectRemoteBackend: (id: string) => Promise<void>;
  onAddRemoteBackend: (draft: AddRemoteBackendDraft) => Promise<void>;
  onMoveRemoteBackend: (id: string, direction: "up" | "down") => Promise<void>;
  onDeleteRemoteBackend: (id: string) => Promise<void>;
  onRefreshTailscaleStatus: () => void;
  onRefreshTailscaleCommandPreview: () => void;
  onUseSuggestedTailscaleHost: () => Promise<void>;
  onTcpDaemonStart: () => Promise<void>;
  onTcpDaemonStop: () => Promise<void>;
  onTcpDaemonStatus: () => Promise<void>;
  onMobileConnectTest: () => void;
};

const formatErrorMessage = (error: unknown, fallback: string) => {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }
  return fallback;
};

type RemoteBackendTarget = AppSettings["remoteBackends"][number];

const createRemoteBackendId = () =>
  `remote-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

const buildFallbackRemoteBackend = (settings: AppSettings): RemoteBackendTarget => ({
  id: settings.activeRemoteBackendId ?? "remote-default",
  name: "Primary remote",
  provider: "tcp",
  host: settings.remoteBackendHost,
  token: settings.remoteBackendToken,
  lastConnectedAtMs: null,
});

const getConfiguredRemoteBackends = (settings: AppSettings): RemoteBackendTarget[] => {
  if (settings.remoteBackends.length > 0) {
    return settings.remoteBackends;
  }
  return [buildFallbackRemoteBackend(settings)];
};

const getActiveRemoteBackend = (settings: AppSettings): RemoteBackendTarget => {
  const configured = getConfiguredRemoteBackends(settings);
  return configured.find((entry) => entry.id === settings.activeRemoteBackendId) ?? configured[0];
};

const validateRemoteHost = (value: string): string | null => {
  const trimmed = value.trim();
  if (!trimmed) {
    return "Host is required.";
  }
  const match = trimmed.match(/^([^:\s]+|\[[^\]]+\]):([0-9]{1,5})$/);
  if (!match) {
    return "Use host:port (for example `macbook.tailnet.ts.net:4732`).";
  }
  const port = Number(match[2]);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    return "Port must be between 1 and 65535.";
  }
  return null;
};

const buildNextRemoteName = (remoteBackends: RemoteBackendTarget[]) => {
  const normalized = new Set(remoteBackends.map((entry) => entry.name.trim().toLowerCase()));
  let index = remoteBackends.length + 1;
  let candidate = `Remote ${index}`;
  while (normalized.has(candidate.toLowerCase())) {
    index += 1;
    candidate = `Remote ${index}`;
  }
  return candidate;
};

export const useSettingsServerSection = ({
  appSettings,
  onUpdateAppSettings,
  onMobileConnectSuccess,
}: UseSettingsServerSectionArgs): SettingsServerSectionProps => {
  const initialActiveRemoteBackend = getActiveRemoteBackend(appSettings);
  const [remoteNameDraft, setRemoteNameDraft] = useState(initialActiveRemoteBackend.name);
  const [remoteHostDraft, setRemoteHostDraft] = useState(initialActiveRemoteBackend.host);
  const [remoteTokenDraft, setRemoteTokenDraft] = useState(initialActiveRemoteBackend.token ?? "");
  const [remoteStatusText, setRemoteStatusText] = useState<string | null>(null);
  const [remoteStatusError, setRemoteStatusError] = useState(false);
  const [remoteNameError, setRemoteNameError] = useState<string | null>(null);
  const [remoteHostError, setRemoteHostError] = useState<string | null>(null);
  const [tcpDaemonBusyAction, setTcpDaemonBusyAction] = useState<
    "start" | "stop" | "status" | null
  >(null);

  const tailscaleStatusQuery = useTauriQuery<TailscaleStatus | null>({
    queryKey: qk.settings.tailscaleStatus(),
    queryFn: () => fetchTailscaleStatus(),
    fallback: null,
  });

  const tailscaleCommandPreviewQuery = useTauriQuery<TailscaleDaemonCommandPreview | null>({
    queryKey: qk.settings.tailscaleCommandPreview(),
    queryFn: () => fetchTailscaleDaemonCommandPreview(),
    fallback: null,
  });

  const tcpDaemonStatusQuery = useTauriQuery<TcpDaemonStatus | null>({
    queryKey: qk.settings.tcpDaemonStatus(),
    queryFn: () => tailscaleDaemonStatus(),
    fallback: null,
  });

  const tcpDaemonStart = useTauriMutation<TcpDaemonStatus, void>({
    mutationFn: () => tailscaleDaemonStart(),
    invalidates: [qk.settings.tcpDaemonStatus()],
  });
  const tcpDaemonStop = useTauriMutation<TcpDaemonStatus, void>({
    mutationFn: () => tailscaleDaemonStop(),
    invalidates: [qk.settings.tcpDaemonStatus()],
  });
  const [mobileConnectBusy, setMobileConnectBusy] = useState(false);
  const [mobileConnectStatusText, setMobileConnectStatusText] = useState<string | null>(null);
  const [mobileConnectStatusError, setMobileConnectStatusError] = useState(false);
  const mobilePlatform = useMemo(() => isMobilePlatform(), []);

  const latestSettingsRef = useRef(appSettings);
  const activeRemoteBackend = useMemo(() => getActiveRemoteBackend(appSettings), [appSettings]);

  const setRemoteStatus = useCallback((message: string | null, isError = false) => {
    setRemoteStatusText(message);
    setRemoteStatusError(isError);
  }, []);

  useEffect(() => {
    latestSettingsRef.current = appSettings;
  }, [appSettings]);

  useEffect(() => {
    setRemoteNameDraft(activeRemoteBackend.name);
    setRemoteHostDraft(activeRemoteBackend.host);
    setRemoteTokenDraft(activeRemoteBackend.token ?? "");
    setRemoteNameError(null);
    setRemoteHostError(null);
  }, [activeRemoteBackend]);

  const normalizeRemoteBackendEntry = (
    entry: RemoteBackendTarget,
    index: number,
  ): RemoteBackendTarget => ({
    id: entry.id?.trim() || `remote-${index + 1}`,
    name: entry.name?.trim() || `Remote ${index + 1}`,
    provider: "tcp",
    host: entry.host?.trim() || DEFAULT_REMOTE_HOST,
    token: entry.token?.trim() ? entry.token.trim() : null,
    lastConnectedAtMs:
      typeof entry.lastConnectedAtMs === "number" && Number.isFinite(entry.lastConnectedAtMs)
        ? entry.lastConnectedAtMs
        : null,
  });

  const buildSettingsFromRemoteBackends = useCallback(
    (
      latestSettings: AppSettings,
      remoteBackends: RemoteBackendTarget[],
      preferredActiveId?: string | null,
    ): AppSettings => {
      const normalizedBackends = remoteBackends.length
        ? remoteBackends.map(normalizeRemoteBackendEntry)
        : [normalizeRemoteBackendEntry(buildFallbackRemoteBackend(latestSettings), 0)];
      const active =
        normalizedBackends.find((entry) => entry.id === preferredActiveId) ??
        normalizedBackends.find((entry) => entry.id === latestSettings.activeRemoteBackendId) ??
        normalizedBackends[0];
      return {
        ...latestSettings,
        remoteBackends: normalizedBackends,
        activeRemoteBackendId: active.id,
        remoteBackendProvider: "tcp",
        remoteBackendHost: active.host,
        remoteBackendToken: active.token,
        ...(mobilePlatform
          ? {
              backendMode: "remote",
            }
          : {}),
      };
    },
    [mobilePlatform],
  );

  const persistRemoteBackends = useCallback(
    async (remoteBackends: RemoteBackendTarget[], preferredActiveId?: string | null) => {
      const latestSettings = latestSettingsRef.current;
      const nextSettings = buildSettingsFromRemoteBackends(
        latestSettings,
        remoteBackends,
        preferredActiveId,
      );
      const unchanged =
        nextSettings.remoteBackendHost === latestSettings.remoteBackendHost &&
        nextSettings.remoteBackendToken === latestSettings.remoteBackendToken &&
        nextSettings.backendMode === latestSettings.backendMode &&
        nextSettings.remoteBackendProvider === latestSettings.remoteBackendProvider &&
        nextSettings.activeRemoteBackendId === latestSettings.activeRemoteBackendId &&
        JSON.stringify(nextSettings.remoteBackends) ===
          JSON.stringify(latestSettings.remoteBackends);
      if (unchanged) {
        return;
      }
      await onUpdateAppSettings(nextSettings);
      latestSettingsRef.current = nextSettings;
    },
    [buildSettingsFromRemoteBackends, onUpdateAppSettings],
  );

  const updateActiveRemoteBackend = useCallback(
    async (patch: Partial<RemoteBackendTarget>) => {
      const latestSettings = latestSettingsRef.current;
      const active = getActiveRemoteBackend(latestSettings);
      const nextBackends = [...getConfiguredRemoteBackends(latestSettings)];
      const activeIndex = nextBackends.findIndex((entry) => entry.id === active.id);
      const safeIndex = activeIndex >= 0 ? activeIndex : 0;
      nextBackends[safeIndex] = {
        ...nextBackends[safeIndex],
        ...patch,
        provider: "tcp",
      };
      await persistRemoteBackends(nextBackends, nextBackends[safeIndex].id);
    },
    [persistRemoteBackends],
  );

  const applyRemoteHost = async (rawValue: string) => {
    const nextHost = rawValue.trim();
    const validationError = validateRemoteHost(nextHost);
    if (validationError) {
      setRemoteHostError(validationError);
      setRemoteStatus(validationError, true);
      return false;
    }
    const normalizedHost = nextHost || DEFAULT_REMOTE_HOST;
    setRemoteHostError(null);
    setRemoteHostDraft(normalizedHost);
    await updateActiveRemoteBackend({ host: normalizedHost });
    setRemoteStatus("Remote host saved.");
    return true;
  };

  const handleCommitRemoteName = async () => {
    const latestSettings = latestSettingsRef.current;
    const active = getActiveRemoteBackend(latestSettings);
    const nextName = remoteNameDraft.trim();
    if (!nextName) {
      const message = "Name is required.";
      setRemoteNameError(message);
      setRemoteStatus(message, true);
      return;
    }
    const duplicate = getConfiguredRemoteBackends(latestSettings).some(
      (entry) =>
        entry.id !== active.id && entry.name.trim().toLowerCase() === nextName.toLowerCase(),
    );
    if (duplicate) {
      const message = `A remote named "${nextName}" already exists.`;
      setRemoteNameError(message);
      setRemoteStatus(message, true);
      return;
    }
    setRemoteNameError(null);
    setRemoteNameDraft(nextName);
    await updateActiveRemoteBackend({ name: nextName });
    setRemoteStatus(`Saved remote name "${nextName}".`);
  };

  const handleCommitRemoteHost = async () => {
    await applyRemoteHost(remoteHostDraft);
  };

  const handleCommitRemoteToken = async () => {
    const nextToken = remoteTokenDraft.trim() ? remoteTokenDraft.trim() : null;
    setRemoteTokenDraft(nextToken ?? "");
    await updateActiveRemoteBackend({ token: nextToken });
    setRemoteStatus("Remote token saved.");
  };

  const handleSelectRemoteBackend = async (id: string) => {
    const latestSettings = latestSettingsRef.current;
    const candidates = getConfiguredRemoteBackends(latestSettings);
    const selected = candidates.find((entry) => entry.id === id);
    if (!selected) {
      return;
    }
    await persistRemoteBackends(candidates, id);
    setRemoteStatus(`Active remote set to "${selected.name}".`);
  };

  const handleAddRemoteBackend = async (draft: AddRemoteBackendDraft) => {
    const latestSettings = latestSettingsRef.current;
    const existingBackends = getConfiguredRemoteBackends(latestSettings);
    const nextName = draft.name.trim();
    if (!nextName) {
      const message = "Name is required.";
      setRemoteStatus(message, true);
      throw new Error(message);
    }
    const duplicate = existingBackends.some(
      (entry) => entry.name.trim().toLowerCase() === nextName.toLowerCase(),
    );
    if (duplicate) {
      const message = `A remote named "${nextName}" already exists.`;
      setRemoteStatus(message, true);
      throw new Error(message);
    }
    const nextHost = draft.host.trim();
    const hostError = validateRemoteHost(nextHost);
    if (hostError) {
      setRemoteStatus(hostError, true);
      throw new Error(hostError);
    }
    const nextToken = draft.token.trim() ? draft.token.trim() : null;
    if (!nextToken) {
      const message = "Remote backend token is required.";
      setRemoteStatus(message, true);
      throw new Error(message);
    }

    const nextId = createRemoteBackendId();
    const nextRemote: RemoteBackendTarget = {
      id: nextId,
      name: nextName,
      provider: "tcp",
      host: nextHost,
      token: nextToken,
      lastConnectedAtMs: null,
    };

    const previousSettings = latestSettings;
    const candidateBackends = [...existingBackends, nextRemote];
    const candidateSettings = buildSettingsFromRemoteBackends(
      previousSettings,
      candidateBackends,
      nextId,
    );

    let candidatePersisted = false;
    try {
      await onUpdateAppSettings(candidateSettings);
      latestSettingsRef.current = candidateSettings;
      candidatePersisted = true;

      const workspaces = await listWorkspaces();
      const workspaceCount = workspaces.length;
      const workspaceWord = workspaceCount === 1 ? "workspace" : "workspaces";
      const connectedBackends = candidateBackends.map((entry) =>
        entry.id === nextId ? { ...entry, lastConnectedAtMs: Date.now() } : entry,
      );
      const connectedSettings = buildSettingsFromRemoteBackends(
        candidateSettings,
        connectedBackends,
        nextId,
      );
      await onUpdateAppSettings(connectedSettings);
      latestSettingsRef.current = connectedSettings;
      setRemoteStatus(
        `Added "${nextName}" and connected. ${workspaceCount} ${workspaceWord} reachable on the remote backend.`,
      );
      await onMobileConnectSuccess?.();
    } catch (error) {
      if (candidatePersisted) {
        try {
          await onUpdateAppSettings(previousSettings);
          latestSettingsRef.current = previousSettings;
        } catch {
          // Keep the original connection error surfaced below.
        }
      }
      const message = formatErrorMessage(error, "Unable to connect to the new remote backend.");
      setRemoteStatus(message, true);
      throw new Error(message);
    }
  };

  const handleSetRemoteNameDraft: Dispatch<SetStateAction<string>> = (value) => {
    setRemoteNameError(null);
    setRemoteStatus(null);
    setRemoteNameDraft((previous) => (typeof value === "function" ? value(previous) : value));
  };

  const handleSetRemoteHostDraft: Dispatch<SetStateAction<string>> = (value) => {
    setRemoteHostError(null);
    setRemoteStatus(null);
    setRemoteHostDraft((previous) => (typeof value === "function" ? value(previous) : value));
  };

  const handleMoveRemoteBackend = async (id: string, direction: "up" | "down") => {
    const latestSettings = latestSettingsRef.current;
    const nextBackends = [...getConfiguredRemoteBackends(latestSettings)];
    const index = nextBackends.findIndex((entry) => entry.id === id);
    if (index < 0) {
      return;
    }
    const targetIndex = direction === "up" ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= nextBackends.length) {
      return;
    }
    const entry = nextBackends[index];
    nextBackends[index] = nextBackends[targetIndex];
    nextBackends[targetIndex] = entry;
    await persistRemoteBackends(nextBackends);
    setRemoteStatus(`Moved "${entry.name}" ${direction}.`);
  };

  const handleDeleteRemoteBackend = async (id: string) => {
    const latestSettings = latestSettingsRef.current;
    const existingBackends = getConfiguredRemoteBackends(latestSettings);
    if (existingBackends.length <= 1) {
      setRemoteStatus("You need at least one remote.", true);
      return;
    }
    const index = existingBackends.findIndex((entry) => entry.id === id);
    if (index < 0) {
      return;
    }
    const removed = existingBackends[index];
    const remaining = existingBackends.filter((entry) => entry.id !== id);
    const nextActiveId =
      latestSettings.activeRemoteBackendId === id
        ? (remaining[Math.min(index, remaining.length - 1)]?.id ?? remaining[0]?.id ?? null)
        : latestSettings.activeRemoteBackendId;
    await persistRemoteBackends(remaining, nextActiveId);
    setRemoteStatus(`Deleted "${removed.name}".`);
  };

  const handleMobileConnectTest = () => {
    void (async () => {
      const nextToken = remoteTokenDraft.trim() ? remoteTokenDraft.trim() : null;
      setRemoteTokenDraft(nextToken ?? "");

      if (!nextToken) {
        setMobileConnectStatusError(true);
        setMobileConnectStatusText("Remote backend token is required.");
        return;
      }

      const hostError = validateRemoteHost(remoteHostDraft);
      if (hostError) {
        setRemoteHostError(hostError);
        setMobileConnectStatusError(true);
        setMobileConnectStatusText(hostError);
        return;
      }

      setMobileConnectBusy(true);
      setMobileConnectStatusText(null);
      setMobileConnectStatusError(false);
      try {
        const nextHost = remoteHostDraft.trim() || DEFAULT_REMOTE_HOST;
        setRemoteHostDraft(nextHost);
        await updateActiveRemoteBackend({
          host: nextHost,
          token: nextToken,
        });

        const workspaces = await listWorkspaces();
        const workspaceCount = workspaces.length;
        const workspaceWord = workspaceCount === 1 ? "workspace" : "workspaces";
        try {
          await updateActiveRemoteBackend({ lastConnectedAtMs: Date.now() });
        } catch {
          // Keep successful connectivity outcome even if timestamp persistence fails.
        }
        setMobileConnectStatusText(
          `Connected. ${workspaceCount} ${workspaceWord} reachable on the remote backend.`,
        );
        await onMobileConnectSuccess?.();
      } catch (error) {
        setMobileConnectStatusError(true);
        setMobileConnectStatusText(
          error instanceof Error ? error.message : "Unable to connect to remote backend.",
        );
      } finally {
        setMobileConnectBusy(false);
      }
    })();
  };

  useEffect(() => {
    if (!mobilePlatform) {
      return;
    }
    setMobileConnectStatusText(null);
    setMobileConnectStatusError(false);
  }, [mobilePlatform, remoteHostDraft, remoteTokenDraft]);

  const handleRefreshTailscaleStatus = () => {
    void tailscaleStatusQuery.refetch();
  };

  const handleRefreshTailscaleCommandPreview = () => {
    void tailscaleCommandPreviewQuery.refetch();
  };

  const handleUseSuggestedTailscaleHost = async () => {
    const suggestedHost = tailscaleStatusQuery.data?.suggestedRemoteHost ?? null;
    if (!suggestedHost) {
      return;
    }
    await applyRemoteHost(suggestedHost);
  };

  const runTcpDaemonAction = async (
    action: "start" | "stop" | "status",
    run: () => Promise<unknown>,
  ) => {
    setTcpDaemonBusyAction(action);
    try {
      await run();
    } catch {
      // errors surface through query/mutation state if needed
    } finally {
      setTcpDaemonBusyAction(null);
    }
  };

  const handleTcpDaemonStart = async () => {
    await runTcpDaemonAction("start", () => tcpDaemonStart.mutate());
  };

  const handleTcpDaemonStop = async () => {
    await runTcpDaemonAction("stop", () => tcpDaemonStop.mutate());
  };

  const handleTcpDaemonStatus = async () => {
    await runTcpDaemonAction("status", () => tcpDaemonStatusQuery.refetch());
  };

  return {
    appSettings,
    onUpdateAppSettings,
    remoteBackends: getConfiguredRemoteBackends(appSettings),
    activeRemoteBackendId:
      appSettings.activeRemoteBackendId ?? getConfiguredRemoteBackends(appSettings)[0]?.id ?? null,
    remoteStatusText,
    remoteStatusError,
    remoteNameError,
    remoteHostError,
    remoteNameDraft,
    remoteHostDraft,
    remoteTokenDraft,
    nextRemoteNameSuggestion: buildNextRemoteName(getConfiguredRemoteBackends(appSettings)),
    tailscaleStatus: tailscaleStatusQuery.data,
    tailscaleStatusBusy: tailscaleStatusQuery.isLoading,
    tailscaleStatusError: tailscaleStatusQuery.error
      ? formatErrorMessage(tailscaleStatusQuery.error, "Unable to load Tailscale status.")
      : null,
    tailscaleCommandPreview: tailscaleCommandPreviewQuery.data,
    tailscaleCommandBusy: tailscaleCommandPreviewQuery.isLoading,
    tailscaleCommandError: tailscaleCommandPreviewQuery.error
      ? formatErrorMessage(
          tailscaleCommandPreviewQuery.error,
          "Unable to build Tailscale daemon command.",
        )
      : null,
    tcpDaemonStatus: tcpDaemonStatusQuery.data,
    tcpDaemonBusyAction,
    onSetRemoteNameDraft: handleSetRemoteNameDraft,
    onSetRemoteHostDraft: handleSetRemoteHostDraft,
    onSetRemoteTokenDraft: setRemoteTokenDraft,
    onCommitRemoteName: handleCommitRemoteName,
    onCommitRemoteHost: handleCommitRemoteHost,
    onCommitRemoteToken: handleCommitRemoteToken,
    onSelectRemoteBackend: handleSelectRemoteBackend,
    onAddRemoteBackend: handleAddRemoteBackend,
    onMoveRemoteBackend: handleMoveRemoteBackend,
    onDeleteRemoteBackend: handleDeleteRemoteBackend,
    onRefreshTailscaleStatus: handleRefreshTailscaleStatus,
    onRefreshTailscaleCommandPreview: handleRefreshTailscaleCommandPreview,
    onUseSuggestedTailscaleHost: handleUseSuggestedTailscaleHost,
    onTcpDaemonStart: handleTcpDaemonStart,
    onTcpDaemonStop: handleTcpDaemonStop,
    onTcpDaemonStatus: handleTcpDaemonStatus,
    isMobilePlatform: mobilePlatform,
    mobileConnectBusy,
    mobileConnectStatusText,
    mobileConnectStatusError,
    onMobileConnectTest: handleMobileConnectTest,
  };
};
