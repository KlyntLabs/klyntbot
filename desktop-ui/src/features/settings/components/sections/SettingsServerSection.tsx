import X from "lucide-react/dist/esm/icons/x";
import type { Dispatch, SetStateAction } from "react";
import { useMemo, useState } from "react";
import { ModalShell } from "@/features/design-system/components/modal/ModalShell";
import {
  SettingsField,
  SettingsFieldLabel,
  SettingsFieldRow,
  SettingsHelpText,
  SettingsInput,
  SettingsSection,
  SettingsSelect,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import type {
  AppSettings,
  TailscaleDaemonCommandPreview,
  TailscaleStatus,
  TcpDaemonStatus,
} from "@/types";
import { cn } from "@/utils/cn";

type AddRemoteBackendDraft = {
  name: string;
  host: string;
  token: string;
};

type SettingsServerSectionProps = {
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

export function SettingsServerSection({
  appSettings,
  onUpdateAppSettings,
  isMobilePlatform,
  mobileConnectBusy,
  mobileConnectStatusText,
  mobileConnectStatusError,
  remoteBackends,
  activeRemoteBackendId,
  remoteStatusText,
  remoteStatusError,
  remoteNameError,
  remoteHostError,
  remoteNameDraft,
  remoteHostDraft,
  remoteTokenDraft,
  nextRemoteNameSuggestion,
  tailscaleStatus,
  tailscaleStatusBusy,
  tailscaleStatusError,
  tailscaleCommandPreview,
  tailscaleCommandBusy,
  tailscaleCommandError,
  tcpDaemonStatus,
  tcpDaemonBusyAction,
  onSetRemoteNameDraft,
  onSetRemoteHostDraft,
  onSetRemoteTokenDraft,
  onCommitRemoteName,
  onCommitRemoteHost,
  onCommitRemoteToken,
  onSelectRemoteBackend,
  onAddRemoteBackend,
  onMoveRemoteBackend,
  onDeleteRemoteBackend,
  onRefreshTailscaleStatus,
  onRefreshTailscaleCommandPreview,
  onUseSuggestedTailscaleHost,
  onTcpDaemonStart,
  onTcpDaemonStop,
  onTcpDaemonStatus,
  onMobileConnectTest,
}: SettingsServerSectionProps) {
  const [pendingDeleteRemoteId, setPendingDeleteRemoteId] = useState<string | null>(null);
  const [addRemoteOpen, setAddRemoteOpen] = useState(false);
  const [addRemoteBusy, setAddRemoteBusy] = useState(false);
  const [addRemoteError, setAddRemoteError] = useState<string | null>(null);
  const [addRemoteNameDraft, setAddRemoteNameDraft] = useState("");
  const [addRemoteHostDraft, setAddRemoteHostDraft] = useState("");
  const [addRemoteTokenDraft, setAddRemoteTokenDraft] = useState("");
  const isMobileSimplified = isMobilePlatform;
  const pendingDeleteRemote = useMemo(
    () =>
      pendingDeleteRemoteId == null
        ? null
        : (remoteBackends.find((entry) => entry.id === pendingDeleteRemoteId) ?? null),
    [pendingDeleteRemoteId, remoteBackends],
  );
  const tcpRunnerStatusText = (() => {
    if (!tcpDaemonStatus) {
      return null;
    }
    if (tcpDaemonStatus.state === "running") {
      return tcpDaemonStatus.pid
        ? `Mobile daemon is running (pid ${tcpDaemonStatus.pid}) on ${tcpDaemonStatus.listenAddr ?? "configured listen address"}.`
        : `Mobile daemon is running on ${tcpDaemonStatus.listenAddr ?? "configured listen address"}.`;
    }
    if (tcpDaemonStatus.state === "error") {
      return tcpDaemonStatus.lastError ?? "Mobile daemon is in an error state.";
    }
    return `Mobile daemon is stopped${tcpDaemonStatus.listenAddr ? ` (${tcpDaemonStatus.listenAddr})` : ""}.`;
  })();

  const openAddRemoteModal = () => {
    setAddRemoteError(null);
    setAddRemoteNameDraft(nextRemoteNameSuggestion);
    setAddRemoteHostDraft(remoteHostDraft);
    setAddRemoteTokenDraft("");
    setAddRemoteOpen(true);
  };

  const closeAddRemoteModal = () => {
    if (addRemoteBusy) {
      return;
    }
    setAddRemoteOpen(false);
    setAddRemoteError(null);
  };

  const handleAddRemoteConfirm = () => {
    void (async () => {
      if (addRemoteBusy) {
        return;
      }
      setAddRemoteBusy(true);
      setAddRemoteError(null);
      try {
        await onAddRemoteBackend({
          name: addRemoteNameDraft,
          host: addRemoteHostDraft,
          token: addRemoteTokenDraft,
        });
        setAddRemoteOpen(false);
      } catch (error) {
        setAddRemoteError(error instanceof Error ? error.message : "Unable to add remote.");
      } finally {
        setAddRemoteBusy(false);
      }
    })();
  };

  return (
    <SettingsSection
      title="Server"
      subtitle={
        isMobileSimplified
          ? "Configure TCP host/token from your desktop setup, then run a connection test."
          : "Configure how Klynt exposes TCP backend access for mobile and remote clients. Desktop usage remains local unless you explicitly connect through remote mode."
      }
    >
      {!isMobileSimplified && (
        <SettingsField>
          <SettingsFieldLabel htmlFor="backend-mode">Backend mode</SettingsFieldLabel>
          <SettingsSelect
            id="backend-mode"
            value={appSettings.backendMode}
            onChange={(event) =>
              void onUpdateAppSettings({
                ...appSettings,
                backendMode: event.target.value as AppSettings["backendMode"],
              })
            }
          >
            <option value="local">Local (default)</option>
            <option value="remote">Remote (daemon)</option>
          </SettingsSelect>
          <SettingsHelpText>
            Local keeps desktop requests in-process. Remote routes desktop requests through the same
            TCP transport path used by mobile clients.
          </SettingsHelpText>
        </SettingsField>
      )}

      {isMobileSimplified && (
        <>
          <SettingsField>
            <SettingsFieldLabel>Saved remotes</SettingsFieldLabel>
            <ul
              className="flex flex-col gap-2"
              aria-label="Saved remotes"
              style={{ listStyle: "none", padding: 0, margin: 0 }}
            >
              {remoteBackends.map((entry, index) => {
                const isActive = entry.id === activeRemoteBackendId;
                return (
                  <li
                    key={entry.id}
                    className={cn(
                      "flex items-center justify-between gap-2.5 px-3 py-2.5 rounded-xl border bg-surface-card",
                      isActive
                        ? "border-[color-mix(in_srgb,var(--border-accent)_70%,var(--border-muted))] shadow-[0_0_0_1px_color-mix(in_srgb,var(--border-accent)_35%,transparent)]"
                        : "border-border-muted",
                    )}
                  >
                    <div className="min-w-0 flex-1 flex flex-col gap-1">
                      <div className="inline-flex items-center gap-2">
                        <div className="text-ui-sm font-semibold text-text-strong">{entry.name}</div>
                        {isActive && (
                          <span className="inline-flex items-center justify-center rounded-full px-2 py-0.5 text-ui-2xs font-bold tracking-wide text-text-strong bg-[color-mix(in_srgb,var(--border-accent)_18%,transparent)] border border-[color-mix(in_srgb,var(--border-accent)_45%,transparent)]">
                            Active
                          </span>
                        )}
                      </div>
                      <div className="text-ui-xs text-text-subtle whitespace-nowrap overflow-hidden text-ellipsis">
                        TCP · {entry.host}
                      </div>
                      <div className="text-ui-2xs text-text-faint">
                        Last connected:{" "}
                        {typeof entry.lastConnectedAtMs === "number"
                          ? new Date(entry.lastConnectedAtMs).toLocaleString()
                          : "Never"}
                      </div>
                    </div>
                    <div className="shrink-0 inline-flex items-center gap-1">
                      <button
                        type="button"
                        className="ghost min-w-[28px] py-1 px-2 text-ui-xs"
                        onClick={() => {
                          void onSelectRemoteBackend(entry.id);
                        }}
                        disabled={isActive}
                        aria-label={`Use ${entry.name} remote`}
                      >
                        {isActive ? "Using" : "Use"}
                      </button>
                      <button
                        type="button"
                        className="ghost min-w-[28px] py-1 px-2 text-ui-xs"
                        onClick={() => {
                          void onMoveRemoteBackend(entry.id, "up");
                        }}
                        disabled={index === 0}
                        aria-label={`Move ${entry.name} up`}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="ghost min-w-[28px] py-1 px-2 text-ui-xs text-status-error"
                        onClick={() => {
                          void onMoveRemoteBackend(entry.id, "down");
                        }}
                        disabled={index === remoteBackends.length - 1}
                        aria-label={`Move ${entry.name} down`}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="ghost min-w-[28px] py-1 px-2 text-ui-xs text-status-error"
                        onClick={() => {
                          setPendingDeleteRemoteId(entry.id);
                        }}
                        aria-label={`Delete ${entry.name}`}
                      >
                        Delete
                      </button>
                    </div>
                  </li>
                );
              })}
            </ul>
            <SettingsFieldRow className="mt-2">
              <button type="button" className="primary py-1.5 px-2.5 text-ui-sm" onClick={openAddRemoteModal}>
                Add remote
              </button>
            </SettingsFieldRow>
            {remoteStatusText && (
              <SettingsHelpText error={remoteStatusError}>{remoteStatusText}</SettingsHelpText>
            )}
            <SettingsHelpText>
              Switch the active remote here. The fields below edit the active entry.
            </SettingsHelpText>
          </SettingsField>

          <SettingsField>
            <SettingsFieldLabel htmlFor="mobile-remote-name">Remote name</SettingsFieldLabel>
            <SettingsInput
              id="mobile-remote-name"
              compact
              value={remoteNameDraft}
              placeholder="My desktop"
              onChange={(event) => onSetRemoteNameDraft(event.target.value)}
              onBlur={() => {
                void onCommitRemoteName();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void onCommitRemoteName();
                }
              }}
            />
            {remoteNameError && <SettingsHelpText error>{remoteNameError}</SettingsHelpText>}
          </SettingsField>
        </>
      )}

      {!isMobileSimplified && (
        <SettingsToggleRow
          title="Keep daemon running after app closes"
          subtitle="If disabled, Klynt stops managed TCP daemon processes before exit."
        >
          <SettingsToggleSwitch
            pressed={appSettings.keepDaemonRunningAfterAppClose}
            onClick={() =>
              void onUpdateAppSettings({
                ...appSettings,
                keepDaemonRunningAfterAppClose: !appSettings.keepDaemonRunningAfterAppClose,
              })
            }
          />
        </SettingsToggleRow>
      )}

      <SettingsField>
        <SettingsFieldLabel>Remote backend</SettingsFieldLabel>
        <SettingsFieldRow>
          <SettingsInput
            compact
            className="flex-1"
            value={remoteHostDraft}
            placeholder="127.0.0.1:4732"
            onChange={(event) => onSetRemoteHostDraft(event.target.value)}
            onBlur={() => {
              void onCommitRemoteHost();
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void onCommitRemoteHost();
              }
            }}
            aria-label="Remote backend host"
          />
          <SettingsInput
            compact
            type="password"
            className="flex-1"
            value={remoteTokenDraft}
            placeholder="Token (required)"
            onChange={(event) => onSetRemoteTokenDraft(event.target.value)}
            onBlur={() => {
              void onCommitRemoteToken();
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void onCommitRemoteToken();
              }
            }}
            aria-label="Remote backend token"
          />
        </SettingsFieldRow>
        {remoteHostError && <SettingsHelpText error>{remoteHostError}</SettingsHelpText>}
        <SettingsHelpText>
          {isMobileSimplified
            ? "Use the Tailscale host from your desktop Klynt app (Server section), for example `macbook.your-tailnet.ts.net:4732`."
            : "This host/token is used by mobile clients and desktop remote-mode testing."}
        </SettingsHelpText>
      </SettingsField>

      {isMobileSimplified && (
        <SettingsField>
          <SettingsFieldLabel>Connection test</SettingsFieldLabel>
          <SettingsFieldRow>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={onMobileConnectTest}
              disabled={mobileConnectBusy}
            >
              {mobileConnectBusy ? "Connecting..." : "Connect & test"}
            </button>
          </SettingsFieldRow>
          {mobileConnectStatusText && (
            <SettingsHelpText error={mobileConnectStatusError}>{mobileConnectStatusText}</SettingsHelpText>
          )}
          <SettingsHelpText>
            Make sure your desktop app daemon is running and reachable on Tailscale, then retry this
            test.
          </SettingsHelpText>
        </SettingsField>
      )}

      {!isMobileSimplified && (
        <SettingsField>
          <SettingsFieldLabel>Mobile access daemon</SettingsFieldLabel>
          <SettingsFieldRow>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={() => {
                void onTcpDaemonStart();
              }}
              disabled={tcpDaemonBusyAction !== null}
            >
              {tcpDaemonBusyAction === "start" ? "Starting..." : "Start daemon"}
            </button>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={() => {
                void onTcpDaemonStop();
              }}
              disabled={tcpDaemonBusyAction !== null}
            >
              {tcpDaemonBusyAction === "stop" ? "Stopping..." : "Stop daemon"}
            </button>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={() => {
                void onTcpDaemonStatus();
              }}
              disabled={tcpDaemonBusyAction !== null}
            >
              {tcpDaemonBusyAction === "status" ? "Refreshing..." : "Refresh status"}
            </button>
          </SettingsFieldRow>
          {tcpRunnerStatusText && <SettingsHelpText>{tcpRunnerStatusText}</SettingsHelpText>}
          {tcpDaemonStatus?.startedAtMs && (
            <SettingsHelpText>
              Started at: {new Date(tcpDaemonStatus.startedAtMs).toLocaleString()}
            </SettingsHelpText>
          )}
          <SettingsHelpText>
            Start this daemon before connecting from iOS. It uses your current token and listens on{" "}
            <code>0.0.0.0:&lt;port&gt;</code>, matching your configured host port.
          </SettingsHelpText>
        </SettingsField>
      )}

      {!isMobileSimplified && (
        <SettingsField>
          <SettingsFieldLabel>Tailscale helper</SettingsFieldLabel>
          <SettingsFieldRow>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={onRefreshTailscaleStatus}
              disabled={tailscaleStatusBusy}
            >
              {tailscaleStatusBusy ? "Checking..." : "Detect Tailscale"}
            </button>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              onClick={onRefreshTailscaleCommandPreview}
              disabled={tailscaleCommandBusy}
            >
              {tailscaleCommandBusy ? "Refreshing..." : "Refresh daemon command"}
            </button>
            <button
              type="button"
              className="primary py-1.5 px-2.5 text-ui-sm"
              disabled={!tailscaleStatus?.suggestedRemoteHost}
              onClick={() => {
                void onUseSuggestedTailscaleHost();
              }}
            >
              Use suggested host
            </button>
          </SettingsFieldRow>
          {tailscaleStatusError && <SettingsHelpText error>{tailscaleStatusError}</SettingsHelpText>}
          {tailscaleStatus && (
            <>
              <SettingsHelpText>{tailscaleStatus.message}</SettingsHelpText>
              <SettingsHelpText>
                {tailscaleStatus.installed
                  ? `Version: ${tailscaleStatus.version ?? "unknown"}`
                  : "Install Tailscale on both desktop and iOS to continue."}
              </SettingsHelpText>
              {tailscaleStatus.suggestedRemoteHost && (
                <SettingsHelpText>
                  Suggested remote host: <code>{tailscaleStatus.suggestedRemoteHost}</code>
                </SettingsHelpText>
              )}
              {tailscaleStatus.tailnetName && (
                <SettingsHelpText>
                  Tailnet: <code>{tailscaleStatus.tailnetName}</code>
                </SettingsHelpText>
              )}
            </>
          )}
          {tailscaleCommandError && <SettingsHelpText error>{tailscaleCommandError}</SettingsHelpText>}
          {tailscaleCommandPreview && (
            <>
              <SettingsHelpText>
                Command template (manual fallback) for starting the daemon:
              </SettingsHelpText>
              <pre className="bg-surface-control rounded-lg p-3 text-ui-xs font-code whitespace-pre-wrap text-text-primary overflow-auto">
                <code>{tailscaleCommandPreview.command}</code>
              </pre>
              {!tailscaleCommandPreview.tokenConfigured && (
                <SettingsHelpText error>
                  Remote backend token is empty. Set one before exposing daemon access.
                </SettingsHelpText>
              )}
            </>
          )}
        </SettingsField>
      )}

      <SettingsHelpText>
        {isMobileSimplified
          ? "Use your own infrastructure only. On iOS, get the Tailscale hostname and token from your desktop Klynt setup."
          : "Mobile access should stay scoped to your own infrastructure (tailnet). Klynt does not provide hosted backend services."}
      </SettingsHelpText>

      {addRemoteOpen && (
        <ModalShell
          className="z-40"
          cardClassName="w-[min(420px,calc(100vw-40px))] p-4 flex flex-col gap-2.5 bg-[#141b27] border border-white/15 rounded-2xl shadow-[0_24px_60px_rgba(0,0,0,0.55)] text-[#eef3ff]"
          onBackdropClick={closeAddRemoteModal}
          ariaLabel="Add remote"
        >
          <div className="flex items-center justify-between gap-2">
            <div className="text-ui-md font-bold text-[#f5f8ff]">Add remote</div>
            <button
              type="button"
              className="ghost icon-button p-1 text-[#dce6f7]"
              onClick={closeAddRemoteModal}
              aria-label="Close add remote modal"
              disabled={addRemoteBusy}
            >
              <X className="w-3.5 h-3.5" aria-hidden />
            </button>
          </div>
          <SettingsField className="mb-0">
            <SettingsFieldLabel htmlFor="settings-add-remote-name">New remote name</SettingsFieldLabel>
            <SettingsInput
              id="settings-add-remote-name"
              compact
              value={addRemoteNameDraft}
              onChange={(event) => setAddRemoteNameDraft(event.target.value)}
              disabled={addRemoteBusy}
            />
          </SettingsField>
          <SettingsField className="mb-0">
            <SettingsFieldLabel htmlFor="settings-add-remote-host">New remote host</SettingsFieldLabel>
            <SettingsInput
              id="settings-add-remote-host"
              compact
              value={addRemoteHostDraft}
              placeholder="macbook.your-tailnet.ts.net:4732"
              onChange={(event) => setAddRemoteHostDraft(event.target.value)}
              disabled={addRemoteBusy}
            />
          </SettingsField>
          <SettingsField className="mb-0">
            <SettingsFieldLabel htmlFor="settings-add-remote-token">New remote token</SettingsFieldLabel>
            <SettingsInput
              id="settings-add-remote-token"
              compact
              type="password"
              value={addRemoteTokenDraft}
              placeholder="Token"
              onChange={(event) => setAddRemoteTokenDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  handleAddRemoteConfirm();
                }
              }}
              disabled={addRemoteBusy}
            />
          </SettingsField>
          {addRemoteError && <SettingsHelpText error>{addRemoteError}</SettingsHelpText>}
          <div className="inline-flex justify-end gap-2 mt-1">
            <button type="button" className="ghost" onClick={closeAddRemoteModal} disabled={addRemoteBusy}>
              Cancel
            </button>
            <button
              type="button"
              className="primary"
              onClick={handleAddRemoteConfirm}
              disabled={addRemoteBusy}
            >
              {addRemoteBusy ? "Connecting..." : "Connect & add"}
            </button>
          </div>
        </ModalShell>
      )}

      {pendingDeleteRemote && (
        <ModalShell
          className="z-40"
          cardClassName="w-[min(380px,calc(100vw-40px))] p-4 flex flex-col gap-2.5 bg-[#141b27] border border-white/15 rounded-2xl shadow-[0_24px_60px_rgba(0,0,0,0.55)] text-[#eef3ff]"
          onBackdropClick={() => setPendingDeleteRemoteId(null)}
          ariaLabel="Delete remote confirmation"
        >
          <div className="text-ui-md font-bold text-[#f5f8ff]">Delete remote?</div>
          <div className="text-ui-sm text-[rgba(232,240,251,0.86)] leading-relaxed">
            Remove <strong>{pendingDeleteRemote.name}</strong> from saved remotes? This only removes
            the profile from this device.
          </div>
          <div className="inline-flex justify-end gap-2 mt-1">
            <button type="button" className="ghost" onClick={() => setPendingDeleteRemoteId(null)}>
              Cancel
            </button>
            <button
              type="button"
              className="primary"
              onClick={() => {
                void onDeleteRemoteBackend(pendingDeleteRemote.id);
                setPendingDeleteRemoteId(null);
              }}
            >
              Delete remote
            </button>
          </div>
        </ModalShell>
      )}
    </SettingsSection>
  );
}
