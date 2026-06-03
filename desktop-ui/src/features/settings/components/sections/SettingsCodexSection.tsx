import Stethoscope from "lucide-react/dist/esm/icons/stethoscope";
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useMemo, useRef } from "react";
import {
  SettingsField,
  SettingsFieldLabel,
  SettingsFieldRow,
  SettingsHelpText,
  SettingsInput,
  SettingsSection,
  SettingsSelect,
  SettingsToggleRow,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import { FileEditorCard } from "@/features/shared/components/FileEditorCard";
import type { AppSettings, CodexDoctorResult, CodexUpdateResult, ModelOption } from "@/types";
import { cn } from "@/utils/cn";

type SettingsCodexSectionProps = {
  appSettings: AppSettings;
  onUpdateAppSettings: (next: AppSettings) => Promise<void>;
  defaultModels: ModelOption[];
  defaultModelsLoading: boolean;
  defaultModelsError: string | null;
  defaultModelsConnectedWorkspaceCount: number;
  onRefreshDefaultModels: () => void;
  codexPathDraft: string;
  codexArgsDraft: string;
  codexDirty: boolean;
  isSavingSettings: boolean;
  doctorState: {
    status: "idle" | "running" | "done";
    result: CodexDoctorResult | null;
  };
  codexUpdateState: {
    status: "idle" | "running" | "done";
    result: CodexUpdateResult | null;
  };
  globalAgentsMeta: string;
  globalAgentsError: string | null;
  globalAgentsContent: string;
  globalAgentsLoading: boolean;
  globalAgentsRefreshDisabled: boolean;
  globalAgentsSaveDisabled: boolean;
  globalAgentsSaveLabel: string;
  globalConfigMeta: string;
  globalConfigError: string | null;
  globalConfigContent: string;
  globalConfigLoading: boolean;
  globalConfigRefreshDisabled: boolean;
  globalConfigSaveDisabled: boolean;
  globalConfigSaveLabel: string;
  onSetCodexPathDraft: Dispatch<SetStateAction<string>>;
  onSetCodexArgsDraft: Dispatch<SetStateAction<string>>;
  onSetGlobalAgentsContent: (value: string) => void;
  onSetGlobalConfigContent: (value: string) => void;
  onBrowseCodex: () => Promise<void>;
  onSaveCodexSettings: () => Promise<void>;
  onRunDoctor: () => Promise<void>;
  onRunCodexUpdate: () => Promise<void>;
  onRefreshGlobalAgents: () => void;
  onSaveGlobalAgents: () => void;
  onRefreshGlobalConfig: () => void;
  onSaveGlobalConfig: () => void;
};

const DEFAULT_REASONING_EFFORT = "medium";

const normalizeEffortValue = (value: unknown): string | null => {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed.toLowerCase() : null;
};

function coerceSavedModelSlug(value: string | null, models: ModelOption[]): string | null {
  const trimmed = (value ?? "").trim();
  if (!trimmed) {
    return null;
  }
  const bySlug = models.find((model) => model.model === trimmed);
  if (bySlug) {
    return bySlug.model;
  }
  const byId = models.find((model) => model.id === trimmed);
  return byId ? byId.model : null;
}

const getReasoningSupport = (model: ModelOption | null): boolean => {
  if (!model) {
    return false;
  }
  return model.supportedReasoningEfforts.length > 0 || model.defaultReasoningEffort !== null;
};

const getReasoningOptions = (model: ModelOption | null): string[] => {
  if (!model) {
    return [];
  }
  const supported = model.supportedReasoningEfforts
    .map((effort) => normalizeEffortValue(effort.reasoningEffort))
    .filter((effort): effort is string => Boolean(effort));
  if (supported.length > 0) {
    return Array.from(new Set(supported));
  }
  const fallback = normalizeEffortValue(model.defaultReasoningEffort);
  return fallback ? [fallback] : [];
};

export function SettingsCodexSection({
  appSettings,
  onUpdateAppSettings,
  defaultModels,
  defaultModelsLoading,
  defaultModelsError,
  defaultModelsConnectedWorkspaceCount,
  onRefreshDefaultModels,
  codexPathDraft,
  codexArgsDraft,
  codexDirty,
  isSavingSettings,
  doctorState,
  codexUpdateState,
  globalAgentsMeta,
  globalAgentsError,
  globalAgentsContent,
  globalAgentsLoading,
  globalAgentsRefreshDisabled,
  globalAgentsSaveDisabled,
  globalAgentsSaveLabel,
  globalConfigMeta,
  globalConfigError,
  globalConfigContent,
  globalConfigLoading,
  globalConfigRefreshDisabled,
  globalConfigSaveDisabled,
  globalConfigSaveLabel,
  onSetCodexPathDraft,
  onSetCodexArgsDraft,
  onSetGlobalAgentsContent,
  onSetGlobalConfigContent,
  onBrowseCodex,
  onSaveCodexSettings,
  onRunDoctor,
  onRunCodexUpdate,
  onRefreshGlobalAgents,
  onSaveGlobalAgents,
  onRefreshGlobalConfig,
  onSaveGlobalConfig,
}: SettingsCodexSectionProps) {
  const latestModelSlug = defaultModels[0]?.model ?? null;
  const savedModelSlug = useMemo(
    () => coerceSavedModelSlug(appSettings.lastComposerModelId, defaultModels),
    [appSettings.lastComposerModelId, defaultModels],
  );
  const selectedModelSlug = savedModelSlug ?? latestModelSlug ?? "";
  const selectedModel = useMemo(
    () => defaultModels.find((model) => model.model === selectedModelSlug) ?? null,
    [defaultModels, selectedModelSlug],
  );
  const reasoningSupported = useMemo(() => getReasoningSupport(selectedModel), [selectedModel]);
  const reasoningOptions = useMemo(() => getReasoningOptions(selectedModel), [selectedModel]);
  const savedEffort = useMemo(
    () => normalizeEffortValue(appSettings.lastComposerReasoningEffort),
    [appSettings.lastComposerReasoningEffort],
  );
  const selectedEffort = useMemo(() => {
    if (!reasoningSupported) {
      return "";
    }
    if (savedEffort && reasoningOptions.includes(savedEffort)) {
      return savedEffort;
    }
    if (reasoningOptions.includes(DEFAULT_REASONING_EFFORT)) {
      return DEFAULT_REASONING_EFFORT;
    }
    const fallback = normalizeEffortValue(selectedModel?.defaultReasoningEffort);
    if (fallback && reasoningOptions.includes(fallback)) {
      return fallback;
    }
    return reasoningOptions[0] ?? "";
  }, [reasoningOptions, reasoningSupported, savedEffort, selectedModel]);

  const didNormalizeDefaultsRef = useRef(false);
  useEffect(() => {
    if (didNormalizeDefaultsRef.current) {
      return;
    }
    if (!defaultModels.length) {
      return;
    }
    const savedRawModel = (appSettings.lastComposerModelId ?? "").trim();
    const savedRawEffort = (appSettings.lastComposerReasoningEffort ?? "").trim();
    const shouldNormalizeModel = savedRawModel.length === 0 || savedModelSlug === null;
    const shouldNormalizeEffort =
      reasoningSupported &&
      (savedRawEffort.length === 0 ||
        savedEffort === null ||
        !reasoningOptions.includes(savedEffort));
    if (!shouldNormalizeModel && !shouldNormalizeEffort) {
      didNormalizeDefaultsRef.current = true;
      return;
    }

    const next: AppSettings = {
      ...appSettings,
      lastComposerModelId: shouldNormalizeModel
        ? selectedModelSlug
        : appSettings.lastComposerModelId,
      lastComposerReasoningEffort: shouldNormalizeEffort
        ? selectedEffort
        : appSettings.lastComposerReasoningEffort,
    };
    didNormalizeDefaultsRef.current = true;
    void onUpdateAppSettings(next);
  }, [
    appSettings,
    defaultModels.length,
    onUpdateAppSettings,
    reasoningOptions,
    reasoningSupported,
    savedEffort,
    savedModelSlug,
    selectedModelSlug,
    selectedEffort,
  ]);

  return (
    <SettingsSection
      title="Codex"
      subtitle="Configure the Codex CLI used by Klynt and validate the install."
    >
      <SettingsField>
        <SettingsFieldLabel htmlFor="codex-path">Default Codex path</SettingsFieldLabel>
        <SettingsFieldRow>
          <SettingsInput
            id="codex-path"
            value={codexPathDraft}
            placeholder="codex"
            onChange={(event) => onSetCodexPathDraft(event.target.value)}
          />
          <button
            type="button"
            className="ghost"
            onClick={() => {
              void onBrowseCodex();
            }}
          >
            Browse
          </button>
          <button type="button" className="ghost" onClick={() => onSetCodexPathDraft("")}>
            Use PATH
          </button>
        </SettingsFieldRow>
        <SettingsHelpText>Leave empty to use the system PATH resolution.</SettingsHelpText>
        <SettingsFieldLabel htmlFor="codex-args">Default Codex args</SettingsFieldLabel>
        <SettingsFieldRow>
          <SettingsInput
            id="codex-args"
            value={codexArgsDraft}
            placeholder="--profile personal"
            onChange={(event) => onSetCodexArgsDraft(event.target.value)}
          />
          <button type="button" className="ghost" onClick={() => onSetCodexArgsDraft("")}>
            Clear
          </button>
        </SettingsFieldRow>
        <SettingsHelpText>
          Extra flags passed before <code>app-server</code>. Use quotes for values with spaces.
        </SettingsHelpText>
        <SettingsHelpText>
          These settings apply to the shared Codex app-server used across all connected workspaces.
        </SettingsHelpText>
        <SettingsHelpText>
          Per-thread override processing ignores unsupported flags: <code>-m</code>/
          <code>--model</code>, <code>-a</code>/<code>--ask-for-approval</code>, <code>-s</code>/
          <code>--sandbox</code>, <code>--full-auto</code>,{" "}
          <code>--dangerously-bypass-approvals-and-sandbox</code>, <code>--oss</code>,{" "}
          <code>--local-provider</code>, and <code>--no-alt-screen</code>.
        </SettingsHelpText>
        <div className="flex gap-2.5 items-center">
          {codexDirty && (
            <button
              type="button"
              className="primary"
              onClick={() => {
                void onSaveCodexSettings();
              }}
              disabled={isSavingSettings}
            >
              {isSavingSettings ? "Saving..." : "Save"}
            </button>
          )}
          <button
            type="button"
            className="ghost py-1.5 px-2.5 text-ui-sm"
            onClick={() => {
              void onRunDoctor();
            }}
            disabled={doctorState.status === "running"}
          >
            <Stethoscope aria-hidden />
            {doctorState.status === "running" ? "Running..." : "Run doctor"}
          </button>
          <button
            type="button"
            className="ghost py-1.5 px-2.5 text-ui-sm"
            onClick={() => {
              void onRunCodexUpdate();
            }}
            disabled={codexUpdateState.status === "running"}
            title="Update Codex"
          >
            <Stethoscope aria-hidden />
            {codexUpdateState.status === "running" ? "Updating..." : "Update"}
          </button>
        </div>

        {doctorState.result && (
          <div
            className={cn(
              "mt-2 p-3 px-3.5 rounded-xl border text-ui-xs",
              doctorState.result.ok
                ? "border-[rgba(120,235,190,0.4)] text-text-strong bg-surface-card"
                : "border-[rgba(255,120,120,0.45)] text-text-strong bg-surface-card",
            )}
          >
            <div className="font-semibold mb-1.5">
              {doctorState.result.ok ? "Codex looks good" : "Codex issue detected"}
            </div>
            <div className="flex flex-col gap-1">
              <div>Version: {doctorState.result.version ?? "unknown"}</div>
              <div>App-server: {doctorState.result.appServerOk ? "ok" : "failed"}</div>
              <div>
                Node:{" "}
                {doctorState.result.nodeOk
                  ? `ok (${doctorState.result.nodeVersion ?? "unknown"})`
                  : "missing"}
              </div>
              {doctorState.result.details && <div>{doctorState.result.details}</div>}
              {doctorState.result.nodeDetails && <div>{doctorState.result.nodeDetails}</div>}
              {doctorState.result.path && (
                <div className="break-all [overflow-wrap:anywhere]">
                  PATH: {doctorState.result.path}
                </div>
              )}
            </div>
          </div>
        )}

        {codexUpdateState.result && (
          <div
            className={cn(
              "mt-2 p-3 px-3.5 rounded-xl border text-ui-xs",
              codexUpdateState.result.ok
                ? "border-[rgba(120,235,190,0.4)] text-text-strong bg-surface-card"
                : "border-[rgba(255,120,120,0.45)] text-text-strong bg-surface-card",
            )}
          >
            <div className="font-semibold mb-1.5">
              {codexUpdateState.result.ok
                ? codexUpdateState.result.upgraded
                  ? "Codex updated"
                  : "Codex already up-to-date"
                : "Codex update failed"}
            </div>
            <div className="flex flex-col gap-1">
              <div>Method: {codexUpdateState.result.method}</div>
              {codexUpdateState.result.package && (
                <div>Package: {codexUpdateState.result.package}</div>
              )}
              <div>
                Version:{" "}
                {codexUpdateState.result.afterVersion ??
                  codexUpdateState.result.beforeVersion ??
                  "unknown"}
              </div>
              {codexUpdateState.result.details && <div>{codexUpdateState.result.details}</div>}
              {codexUpdateState.result.output && (
                <details>
                  <summary>output</summary>
                  <pre>{codexUpdateState.result.output}</pre>
                </details>
              )}
            </div>
          </div>
        )}
      </SettingsField>

      <div className="h-px bg-border-muted my-4 rounded-full" />
      <div className="text-ui-sm font-semibold text-text-strong mb-2.5">Default parameters</div>

      <SettingsToggleRow
        title={<label htmlFor="default-model">Model</label>}
        subtitle={
          defaultModelsConnectedWorkspaceCount === 0
            ? "Add a workspace to load available models."
            : defaultModelsLoading
              ? "Loading models from the first workspace…"
              : defaultModelsError
                ? `Couldn’t load models: ${defaultModelsError}`
                : "Sourced from the first workspace and used when there is no thread-specific override."
        }
      >
        <SettingsFieldRow>
          <SettingsSelect
            id="default-model"
            value={selectedModelSlug}
            disabled={!defaultModels.length || defaultModelsLoading}
            onChange={(event) =>
              void onUpdateAppSettings({
                ...appSettings,
                lastComposerModelId: event.target.value,
              })
            }
            aria-label="Model"
          >
            {defaultModels.map((model) => (
              <option key={model.model} value={model.model}>
                {model.displayName?.trim() || model.model}
              </option>
            ))}
          </SettingsSelect>
          <button
            type="button"
            className="ghost"
            onClick={onRefreshDefaultModels}
            disabled={defaultModelsLoading || defaultModelsConnectedWorkspaceCount === 0}
          >
            Refresh
          </button>
        </SettingsFieldRow>
      </SettingsToggleRow>

      <SettingsToggleRow
        title={<label htmlFor="default-effort">Reasoning effort</label>}
        subtitle={
          reasoningSupported
            ? "Available options depend on the selected model."
            : "The selected model does not expose reasoning effort options."
        }
      >
        <SettingsSelect
          id="default-effort"
          value={selectedEffort}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              lastComposerReasoningEffort: event.target.value,
            })
          }
          aria-label="Reasoning effort"
          disabled={!reasoningSupported}
        >
          {!reasoningSupported && <option value="">not supported</option>}
          {reasoningOptions.map((effort) => (
            <option key={effort} value={effort}>
              {effort}
            </option>
          ))}
        </SettingsSelect>
      </SettingsToggleRow>

      <SettingsToggleRow
        title={<label htmlFor="default-access">Access mode</label>}
        subtitle="Used when there is no thread-specific override."
      >
        <SettingsSelect
          id="default-access"
          value={appSettings.defaultAccessMode}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              defaultAccessMode: event.target.value as AppSettings["defaultAccessMode"],
            })
          }
        >
          <option value="read-only">Read only</option>
          <option value="current">On-request</option>
          <option value="full-access">Full access</option>
        </SettingsSelect>
      </SettingsToggleRow>
      <SettingsField>
        <SettingsFieldLabel htmlFor="review-delivery">Review mode</SettingsFieldLabel>
        <SettingsSelect
          id="review-delivery"
          value={appSettings.reviewDeliveryMode}
          onChange={(event) =>
            void onUpdateAppSettings({
              ...appSettings,
              reviewDeliveryMode: event.target.value as AppSettings["reviewDeliveryMode"],
            })
          }
        >
          <option value="inline">Inline (same thread)</option>
          <option value="detached">Detached (new review thread)</option>
        </SettingsSelect>
        <SettingsHelpText>
          Choose whether <code>/review</code> runs in the current thread or a detached review
          thread.
        </SettingsHelpText>
      </SettingsField>

      <FileEditorCard
        title="Global AGENTS.md"
        meta={globalAgentsMeta}
        error={globalAgentsError}
        value={globalAgentsContent}
        placeholder="Add global instructions for Codex agents…"
        disabled={globalAgentsLoading}
        refreshDisabled={globalAgentsRefreshDisabled}
        saveDisabled={globalAgentsSaveDisabled}
        saveLabel={globalAgentsSaveLabel}
        onChange={onSetGlobalAgentsContent}
        onRefresh={onRefreshGlobalAgents}
        onSave={onSaveGlobalAgents}
        helpText={
          <>
            Stored at <code>~/.codex/AGENTS.md</code>.
          </>
        }
        classNames={{
          container: "flex flex-col gap-2.5 mb-4.5",
          header: "flex items-center justify-between gap-2.5",
          title: "text-ui-sm font-semibold text-text-strong",
          actions: "inline-flex items-center flex-wrap gap-1.5",
          meta: "text-ui-xs text-text-subtle mr-1",
          iconButton: "ghost w-7 h-7 p-0 inline-flex items-center justify-center rounded-lg",
          error:
            "text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]",
          textarea:
            "w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]",
          help: "text-ui-xs text-text-subtle",
        }}
      />

      <FileEditorCard
        title="Global config.toml"
        meta={globalConfigMeta}
        error={globalConfigError}
        value={globalConfigContent}
        placeholder="Edit the global Codex config.toml…"
        disabled={globalConfigLoading}
        refreshDisabled={globalConfigRefreshDisabled}
        saveDisabled={globalConfigSaveDisabled}
        saveLabel={globalConfigSaveLabel}
        onChange={onSetGlobalConfigContent}
        onRefresh={onRefreshGlobalConfig}
        onSave={onSaveGlobalConfig}
        helpText={
          <>
            Stored at <code>~/.codex/config.toml</code>.
          </>
        }
        classNames={{
          container: "flex flex-col gap-2.5 mb-4.5",
          header: "flex items-center justify-between gap-2.5",
          title: "text-ui-sm font-semibold text-text-strong",
          actions: "inline-flex items-center flex-wrap gap-1.5",
          meta: "text-ui-xs text-text-subtle mr-1",
          iconButton: "ghost w-7 h-7 p-0 inline-flex items-center justify-center rounded-lg",
          error:
            "text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]",
          textarea:
            "w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]",
          help: "text-ui-xs text-text-subtle",
        }}
      />
    </SettingsSection>
  );
}
