import type { CodexArgsOption } from "@threads/utils/codexArgsProfiles";
import BrainCog from "lucide-react/dist/esm/icons/brain-cog";
import Server from "lucide-react/dist/esm/icons/server";
import SlidersHorizontal from "lucide-react/dist/esm/icons/sliders-horizontal";
import Zap from "lucide-react/dist/esm/icons/zap";
import type { CSSProperties, ReactNode } from "react";
import type { ProviderInfo } from "@/features/models/hooks/useProviders";
import type { AccessMode, ServiceTier } from "@/types";

type ComposerMetaBarProps = {
  disabled: boolean;
  collaborationModes: { id: string; label: string }[];
  selectedCollaborationModeId: string | null;
  onSelectCollaborationMode: (id: string | null) => void;
  providers: ProviderInfo[];
  selectedProviderId: string | null;
  onSelectProvider: (id: string | null) => void;
  models: { id: string; displayName: string; model: string; provider: string | null }[];
  selectedModelId: string | null;
  onSelectModel: (id: string) => void;
  reasoningOptions: string[];
  selectedEffort: string | null;
  onSelectEffort: (effort: string) => void;
  selectedServiceTier: ServiceTier | null;
  reasoningSupported: boolean;
  accessMode: AccessMode;
  onSelectAccessMode: (mode: AccessMode) => void;
  codexArgsOptions?: CodexArgsOption[];
  selectedCodexArgsOverride?: string | null;
  onSelectCodexArgsOverride?: (value: string | null) => void;
  children?: ReactNode;
};

export function ComposerMetaBar({
  disabled,
  collaborationModes,
  selectedCollaborationModeId,
  onSelectCollaborationMode,
  providers,
  selectedProviderId,
  onSelectProvider,
  models,
  selectedModelId,
  onSelectModel,
  reasoningOptions,
  selectedEffort,
  onSelectEffort,
  selectedServiceTier,
  reasoningSupported,
  accessMode,
  onSelectAccessMode,
  codexArgsOptions = [],
  selectedCodexArgsOverride = null,
  onSelectCodexArgsOverride,
  children,
}: ComposerMetaBarProps) {
  const selectedModel = models.find((model) => model.id === selectedModelId) ?? null;
  const selectedModelLabel = selectedModel?.displayName || selectedModel?.model || "No models";
  const modelSelectStyle = {
    width: `${Math.max(selectedModelLabel.length + 2, 8)}ch`,
  } as CSSProperties;
  const planMode = collaborationModes.find((mode) => mode.id === "plan") ?? null;
  const defaultMode = collaborationModes.find((mode) => mode.id === "default") ?? null;
  const canUsePlanToggle =
    Boolean(planMode) &&
    collaborationModes.every((mode) => mode.id === "default" || mode.id === "plan");
  const planSelected = selectedCollaborationModeId === (planMode?.id ?? "");

  return (
    <div className="composer-bar flex items-center p-[4px_4px_2px] gap-3">
      <div className="composer-meta flex gap-2 flex-wrap">
        {children}
        {collaborationModes.length > 0 &&
          (canUsePlanToggle ? (
            <div className="composer-select-wrap relative inline-flex items-center gap-1.5 rounded-full bg-[var(--cm-surface-panel-strong)] w-max px-2 py-0.5">
              <label className="composer-plan-toggle inline-flex items-center gap-1.5 cursor-pointer select-none" aria-label="Plan mode">
                <input
                  className="composer-plan-toggle-input m-0 w-3 h-3 cursor-pointer accent-[var(--message-link-color)]"
                  type="checkbox"
                  checked={planSelected}
                  disabled={disabled}
                  onChange={(event) =>
                    onSelectCollaborationMode(
                      event.target.checked ? (planMode?.id ?? "plan") : (defaultMode?.id ?? null),
                    )
                  }
                />
                <span className="composer-plan-toggle-icon inline-flex w-4 h-4 text-text-muted" aria-hidden>
                  <svg viewBox="0 0 24 24" fill="none">
                    <title>Plan mode</title>
                    <path
                      d="m6.5 7.5 1 1 2-2M6.5 12.5l1 1 2-2M6.5 17.5l1 1 2-2M11 7.5h7M11 12.5h7M11 17.5h7"
                      stroke="currentColor"
                      strokeWidth="1.4"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </span>
                <span className="composer-plan-toggle-label text-ui-xs text-text-muted leading-none">{planMode?.label || "Plan"}</span>
              </label>
            </div>
          ) : (
            <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
              <span className="composer-icon inline-flex w-3.5 h-3.5 text-text-muted" aria-hidden>
                <svg viewBox="0 0 24 24" fill="none">
                  <title>Collaboration mode</title>
                  <path
                    d="m6.5 7.5 1 1 2-2M6.5 12.5l1 1 2-2M6.5 17.5l1 1 2-2M11 7.5h7M11 12.5h7M11 17.5h7"
                    stroke="currentColor"
                    strokeWidth="1.4"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </span>
              <select
                className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0 w-[78px] max-w-[78px]"
                aria-label="Collaboration mode"
                value={selectedCollaborationModeId ?? ""}
                onChange={(event) => onSelectCollaborationMode(event.target.value || null)}
                disabled={disabled}
              >
                {collaborationModes.map((mode) => (
                  <option key={mode.id} value={mode.id}>
                    {mode.label || mode.id}
                  </option>
                ))}
              </select>
            </div>
          ))}
        {providers.length > 0 && (
          <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
            <span className="composer-icon inline-flex w-3.5 h-3.5 text-text-muted" aria-hidden>
              <Server size={14} strokeWidth={1.8} />
            </span>
            <select
              className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0"
              aria-label="Provider"
              value={selectedProviderId ?? ""}
              onChange={(event) => onSelectProvider(event.target.value || null)}
              disabled={disabled}
            >
              {!selectedProviderId && <option value="">Loading…</option>}
              {providers.map((provider) => (
                <option key={provider.id} value={provider.id}>
                  {provider.displayName}
                </option>
              ))}
            </select>
          </div>
        )}
        <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
          <span className="composer-icon inline-flex w-4 h-4 text-text-muted translate-y-px [&>svg]:!w-4 [&>svg]:!h-4" aria-hidden>
            <svg viewBox="0 0 24 24" fill="none">
              <title>Model</title>
              <path d="M12 4v2" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              <path
                d="M8 7.5h8a2.5 2.5 0 0 1 2.5 2.5v5a2.5 2.5 0 0 1-2.5 2.5H8A2.5 2.5 0 0 1 5.5 15v-5A2.5 2.5 0 0 1 8 7.5Z"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinejoin="round"
              />
              <circle cx="9.5" cy="12.5" r="1" fill="currentColor" />
              <circle cx="14.5" cy="12.5" r="1" fill="currentColor" />
              <path d="M9.5 15.5h5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              <path
                d="M5.5 11H4M20 11h-1.5"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
              />
            </svg>
          </span>
          <select
            className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0"
            aria-label="Model"
            value={selectedModelId ?? ""}
            onChange={(event) => onSelectModel(event.target.value)}
            disabled={disabled || models.length === 0}
            style={modelSelectStyle}
          >
            {models.length === 0 ? (
              <option value="">No models available</option>
            ) : (
              <>
                {!selectedModelId && <option value="">Select model…</option>}
                {models.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName || model.model}
                  </option>
                ))}
              </>
            )}
          </select>
          {selectedServiceTier === "fast" && (
            <span
              className="inline-flex items-center justify-center w-4 h-4 rounded-full text-[var(--accent-warning-strong,#b45309)] bg-[color-mix(in_srgb,var(--accent-warning-strong,#b45309)_16%,transparent)]"
              role="status"
              aria-label="Fast mode enabled"
              title="Fast mode enabled"
            >
              <Zap size={12} strokeWidth={1.8} />
            </span>
          )}
        </div>
        <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
          <span className="composer-icon inline-flex w-4 h-4 text-text-muted translate-y-px" aria-hidden>
            <BrainCog size={14} strokeWidth={1.8} />
          </span>
          <select
            className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0 w-[80px] max-w-[80px]"
            aria-label="Thinking mode"
            value={selectedEffort ?? ""}
            onChange={(event) => onSelectEffort(event.target.value)}
            disabled={disabled || !reasoningSupported}
          >
            {reasoningOptions.length === 0 && <option value="">Default</option>}
            {reasoningOptions.map((effort) => (
              <option key={effort} value={effort}>
                {effort}
              </option>
            ))}
          </select>
        </div>
        {codexArgsOptions.length > 1 && onSelectCodexArgsOverride && (
          <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
            <span className="composer-icon inline-flex w-3.5 h-3.5 text-text-muted" aria-hidden>
              <SlidersHorizontal size={14} strokeWidth={1.8} />
            </span>
            <select
              className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0 w-[90px] max-w-[90px]"
              aria-label="Codex args profile"
              disabled={disabled}
              value={selectedCodexArgsOverride ?? ""}
              onChange={(event) => onSelectCodexArgsOverride(event.target.value || null)}
            >
              {codexArgsOptions.map((option) => (
                <option key={option.value || "default"} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        )}
        <div className="composer-select-wrap relative inline-flex items-center gap-1.5 px-2 py-1 rounded-full bg-[var(--cm-surface-panel-strong)] w-max">
          <span className="composer-icon inline-flex w-3.5 h-3.5 text-text-muted" aria-hidden>
            <svg viewBox="0 0 24 24" fill="none">
              <title>Access mode</title>
              <path
                d="M12 4l7 3v5c0 4.5-3 7.5-7 8-4-0.5-7-3.5-7-8V7l7-3z"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinejoin="round"
              />
              <path
                d="M9.5 12.5l1.8 1.8 3.7-4"
                stroke="currentColor"
                strokeWidth="1.4"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>
          <select
            className="composer-select appearance-none bg-transparent text-text-muted text-ui-xs py-0.5 pr-[18px] pl-0 cursor-pointer w-auto min-w-0 overflow-hidden text-ellipsis whitespace-nowrap border-0 w-[90px] max-w-[90px]"
            aria-label="Agent access"
            disabled={disabled}
            value={accessMode}
            onChange={(event) => onSelectAccessMode(event.target.value as AccessMode)}
          >
            <option value="read-only">Read only</option>
            <option value="current">On-Request</option>
            <option value="full-access">Full access</option>
          </select>
        </div>
      </div>
    </div>
  );
}
