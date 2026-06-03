import type { SettingsAgentsSectionProps } from "@settings/hooks/useSettingsAgentsSection";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { fileManagerName, openInFileManagerLabel } from "@utils/platformPaths";
import { useEffect, useMemo, useState } from "react";
import {
  SettingsSection,
  SettingsSubsection,
  SettingsToggleRow,
  SettingsToggleSwitch,
} from "@/features/design-system/components/settings/SettingsPrimitives";
import {
  MagicSparkleIcon,
  MagicSparkleLoaderIcon,
} from "@/features/shared/components/MagicSparkleIcon";
import type { ModelOption } from "@/types";

const FALLBACK_AGENT_MODELS: ModelOption[] = [
  {
    id: "gpt-5-codex",
    model: "gpt-5-codex",
    displayName: "gpt-5-codex",
    description: "Fallback model while workspace model list is unavailable.",
    provider: "openai",
    supportedReasoningEfforts: [
      { reasoningEffort: "low", description: "" },
      { reasoningEffort: "medium", description: "" },
      { reasoningEffort: "high", description: "" },
    ],
    defaultReasoningEffort: "medium",
    isDefault: true,
  },
];

const MIN_MAX_THREADS = 1;
const MAX_MAX_THREADS = 12;
const MIN_MAX_DEPTH = 1;
const MAX_MAX_DEPTH = 4;

export function SettingsAgentsSection({
  settings,
  isLoading,
  isUpdatingCore,
  creatingAgent,
  updatingAgentName,
  deletingAgentName,
  readingConfigAgentName,
  writingConfigAgentName,
  createDescriptionGenerating,
  editDescriptionGenerating,
  error,
  onRefresh,
  onSetMultiAgentEnabled,
  onSetMaxThreads,
  onSetMaxDepth,
  onCreateAgent,
  onUpdateAgent,
  onDeleteAgent,
  onReadAgentConfig,
  onWriteAgentConfig,
  onGenerateCreateDescription,
  onGenerateEditDescription,
  modelOptions,
  modelOptionsLoading,
  modelOptionsError,
}: SettingsAgentsSectionProps) {
  const [openPathError, setOpenPathError] = useState<string | null>(null);
  const [maxThreadsDraft, setMaxThreadsDraft] = useState("6");
  const [maxDepthDraft, setMaxDepthDraft] = useState("1");

  const [createName, setCreateName] = useState("");
  const [createDescription, setCreateDescription] = useState("");
  const [createDeveloperInstructions, setCreateDeveloperInstructions] = useState("");
  const [createModel, setCreateModel] = useState("");
  const [createReasoningEffort, setCreateReasoningEffort] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);

  const [editingName, setEditingName] = useState<string | null>(null);
  const [pendingDeleteAgentName, setPendingDeleteAgentName] = useState<string | null>(null);
  const [editNameDraft, setEditNameDraft] = useState("");
  const [editDescriptionDraft, setEditDescriptionDraft] = useState("");
  const [editDeveloperInstructionsDraft, setEditDeveloperInstructionsDraft] = useState("");
  const [renameManagedFile, setRenameManagedFile] = useState(true);
  const [editError, setEditError] = useState<string | null>(null);

  const [configEditorAgentName, setConfigEditorAgentName] = useState<string | null>(null);
  const [configEditorContent, setConfigEditorContent] = useState("");
  const [configEditorDirty, setConfigEditorDirty] = useState(false);
  const canGenerateCreateFromName = createName.trim().length > 0;
  const effectiveModelOptions = modelOptions.length > 0 ? modelOptions : FALLBACK_AGENT_MODELS;

  useEffect(() => {
    if (!settings) {
      return;
    }
    setMaxThreadsDraft(String(settings.maxThreads));
    setMaxDepthDraft(String(settings.maxDepth));
  }, [settings]);

  const parseMaxThreads = (rawValue: string): number | null => {
    const value = Number.parseInt(rawValue.trim(), 10);
    if (!Number.isFinite(value) || value < MIN_MAX_THREADS || value > MAX_MAX_THREADS) {
      return null;
    }
    return value;
  };

  const parseMaxDepth = (rawValue: string): number | null => {
    const value = Number.parseInt(rawValue.trim(), 10);
    if (!Number.isFinite(value) || value < MIN_MAX_DEPTH || value > MAX_MAX_DEPTH) {
      return null;
    }
    return value;
  };

  const selectedCreateModel = useMemo(
    () => effectiveModelOptions.find((option) => option.model === createModel) ?? null,
    [createModel, effectiveModelOptions],
  );

  const createReasoningOptions = useMemo(() => {
    if (!selectedCreateModel) {
      return [];
    }
    const supported = selectedCreateModel.supportedReasoningEfforts
      .map((option) => option.reasoningEffort.trim().toLowerCase())
      .filter((value) => value.length > 0);
    if (supported.length > 0) {
      return Array.from(new Set(supported));
    }
    const fallback = selectedCreateModel.defaultReasoningEffort?.trim().toLowerCase() ?? "";
    return fallback ? [fallback] : [];
  }, [selectedCreateModel]);

  useEffect(() => {
    if (!effectiveModelOptions.length) {
      return;
    }
    if (!createModel || !effectiveModelOptions.some((option) => option.model === createModel)) {
      setCreateModel(effectiveModelOptions[0].model);
    }
  }, [createModel, effectiveModelOptions]);

  useEffect(() => {
    if (createReasoningOptions.length === 0) {
      setCreateReasoningEffort("");
      return;
    }
    if (!createReasoningOptions.includes(createReasoningEffort)) {
      if (createReasoningOptions.includes("medium")) {
        setCreateReasoningEffort("medium");
      } else {
        setCreateReasoningEffort(createReasoningOptions[0]);
      }
    }
  }, [createReasoningEffort, createReasoningOptions]);

  const handleOpenPath = async (path: string) => {
    setOpenPathError(null);
    try {
      await revealItemInDir(path);
    } catch (openError) {
      setOpenPathError(openError instanceof Error ? openError.message : "Unable to open path.");
    }
  };

  const handleToggleMultiAgent = async () => {
    if (!settings) {
      return;
    }
    await onSetMultiAgentEnabled(!settings.multiAgentEnabled);
  };

  const handleMaxThreadsChange = async (rawValue: string) => {
    setMaxThreadsDraft(rawValue);
    const parsed = parseMaxThreads(rawValue);
    if (parsed == null) {
      setCreateError(null);
      setEditError(null);
      setOpenPathError(
        `Max threads must be an integer between ${MIN_MAX_THREADS} and ${MAX_MAX_THREADS}.`,
      );
      return;
    }
    setOpenPathError(null);
    if (settings && parsed !== settings.maxThreads) {
      await onSetMaxThreads(parsed);
    }
  };

  const currentMaxThreads = settings
    ? (parseMaxThreads(maxThreadsDraft) ?? settings.maxThreads)
    : MIN_MAX_THREADS;

  const handleMaxThreadsStep = async (delta: number) => {
    if (!settings || isUpdatingCore) {
      return;
    }
    const nextValue = Math.min(
      MAX_MAX_THREADS,
      Math.max(MIN_MAX_THREADS, currentMaxThreads + delta),
    );
    if (nextValue === currentMaxThreads) {
      return;
    }
    await handleMaxThreadsChange(String(nextValue));
  };

  const handleMaxDepthChange = async (rawValue: string) => {
    setMaxDepthDraft(rawValue);
    const parsed = parseMaxDepth(rawValue);
    if (parsed == null) {
      setCreateError(null);
      setEditError(null);
      setOpenPathError(
        `Max depth must be an integer between ${MIN_MAX_DEPTH} and ${MAX_MAX_DEPTH}.`,
      );
      return;
    }
    setOpenPathError(null);
    if (settings && parsed !== settings.maxDepth) {
      await onSetMaxDepth(parsed);
    }
  };

  const currentMaxDepth = settings
    ? (parseMaxDepth(maxDepthDraft) ?? settings.maxDepth)
    : MIN_MAX_DEPTH;

  const handleMaxDepthStep = async (delta: number) => {
    if (!settings || isUpdatingCore) {
      return;
    }
    const nextValue = Math.min(MAX_MAX_DEPTH, Math.max(MIN_MAX_DEPTH, currentMaxDepth + delta));
    if (nextValue === currentMaxDepth) {
      return;
    }
    await handleMaxDepthChange(String(nextValue));
  };

  const handleCreateAgent = async () => {
    const name = createName.trim();
    if (!name) {
      setCreateError("Agent name is required.");
      return;
    }
    setCreateError(null);
    const success = await onCreateAgent({
      name,
      description: createDescription.trim() || null,
      developerInstructions: createDeveloperInstructions.trim() || null,
      template: "blank",
      model: createModel || null,
      reasoningEffort: createReasoningEffort || null,
    });
    if (success) {
      setCreateName("");
      setCreateDescription("");
      setCreateDeveloperInstructions("");
      setCreateReasoningEffort("");
    }
  };

  const startEditing = (
    agent: NonNullable<SettingsAgentsSectionProps["settings"]>["agents"][number],
  ) => {
    setEditingName(agent.name);
    setEditNameDraft(agent.name);
    setEditDescriptionDraft(agent.description ?? "");
    setEditDeveloperInstructionsDraft(agent.developerInstructions ?? "");
    setRenameManagedFile(true);
    setEditError(null);
  };

  const handleUpdateAgent = async () => {
    if (!editingName) {
      return;
    }
    const nextName = editNameDraft.trim();
    if (!nextName) {
      setEditError("Agent name is required.");
      return;
    }
    const editingAgent = settings?.agents.find((agent) => agent.name === editingName) ?? null;
    const previousDeveloperInstructions = editingAgent?.developerInstructions?.trim() ?? "";
    const nextDeveloperInstructions = editDeveloperInstructionsDraft.trim();
    const developerInstructionsChanged =
      nextDeveloperInstructions !== previousDeveloperInstructions;
    setEditError(null);
    const updateInput: Parameters<typeof onUpdateAgent>[0] = {
      originalName: editingName,
      name: nextName,
      description: editDescriptionDraft.trim() || null,
      renameManagedFile,
    };
    if (developerInstructionsChanged) {
      // Send only when explicitly changed so metadata edits don't depend on parsing
      // potentially-invalid per-agent config TOML files.
      updateInput.developerInstructions = editDeveloperInstructionsDraft;
    }
    const success = await onUpdateAgent(updateInput);
    if (success) {
      if (configEditorAgentName === editingName) {
        setConfigEditorAgentName(nextName);
      }
      setEditingName(null);
    }
  };

  const handleDeleteAgent = (name: string) => {
    setPendingDeleteAgentName(name);
  };

  const handleConfirmDeleteAgent = async (name: string) => {
    const success = await onDeleteAgent({ name, deleteManagedFile: true });
    if (!success) {
      return;
    }
    setPendingDeleteAgentName((current) => (current === name ? null : current));
    if (configEditorAgentName === name) {
      setConfigEditorAgentName(null);
      setConfigEditorContent("");
      setConfigEditorDirty(false);
    }
  };

  const handleOpenConfigEditor = async (agentName: string) => {
    const content = await onReadAgentConfig(agentName);
    if (content == null) {
      return;
    }
    setConfigEditorAgentName(agentName);
    setConfigEditorContent(content);
    setConfigEditorDirty(false);
  };

  const handleSaveConfigEditor = async () => {
    if (!configEditorAgentName) {
      return;
    }
    const success = await onWriteAgentConfig(configEditorAgentName, configEditorContent);
    if (success) {
      setConfigEditorDirty(false);
    }
  };

  return (
    <SettingsSection
      title="Agents"
      subtitle="Configure multi-agent mode, limits, and custom agent roles."
    >
      <div className="text-ui-xs text-text-subtle mb-2">
        Built-in roles from Codex are still available: <code>default</code>, <code>explorer</code>,
        and <code>worker</code>.
      </div>

      <SettingsToggleRow
        title="Config file"
        subtitle={<>Open global Codex config in {fileManagerName()}.</>}
      >
        <div className="inline-flex items-center flex-wrap gap-1.5">
          <button type="button" className="ghost" onClick={onRefresh} disabled={isLoading}>
            Refresh
          </button>
          <button
            type="button"
            className="ghost"
            onClick={() => settings && void handleOpenPath(settings.configPath)}
            disabled={!settings}
          >
            {openInFileManagerLabel()}
          </button>
        </div>
      </SettingsToggleRow>

      <SettingsToggleRow
        title="Enable Multi-Agent"
        subtitle={
          <>
            Writes <code>features.multi_agent</code> in config.toml.
          </>
        }
      >
        <SettingsToggleSwitch
          pressed={settings?.multiAgentEnabled ?? false}
          onClick={() => void handleToggleMultiAgent()}
          disabled={!settings || isUpdatingCore}
        />
      </SettingsToggleRow>

      <SettingsToggleRow
        title="Max Threads"
        subtitle={
          <>
            Maximum open agent threads. Valid range: <code>1-12</code>. Changes save immediately.
          </>
        }
      >
        <fieldset className="inline-flex items-center gap-2.5 px-1.5 py-1 rounded-xl border border-border-muted bg-surface-control" aria-label="Maximum agent threads">
          <button
            type="button"
            className="ghost w-7 h-7 p-0 text-ui-md font-bold inline-flex items-center justify-center"
            onClick={() => {
              void handleMaxThreadsStep(-1);
            }}
            disabled={!settings || isUpdatingCore || currentMaxThreads <= MIN_MAX_THREADS}
            aria-label="Decrease max threads"
          >
            ▼
          </button>
          <div className="min-w-[2ch] text-center text-ui-sm font-bold text-text-strong" aria-live="polite" aria-atomic="true">
            {currentMaxThreads}
          </div>
          <button
            type="button"
            className="ghost w-7 h-7 p-0 text-ui-md font-bold inline-flex items-center justify-center"
            onClick={() => {
              void handleMaxThreadsStep(1);
            }}
            disabled={!settings || isUpdatingCore || currentMaxThreads >= MAX_MAX_THREADS}
            aria-label="Increase max threads"
          >
            ▲
          </button>
        </fieldset>
      </SettingsToggleRow>

      <SettingsToggleRow
        title="Max Depth"
        subtitle={
          <>
            Maximum nested spawn depth. Valid range: <code>1-4</code>. Changes save immediately.
          </>
        }
      >
        <fieldset className="inline-flex items-center gap-2.5 px-1.5 py-1 rounded-xl border border-border-muted bg-surface-control" aria-label="Maximum agent depth">
          <button
            type="button"
            className="ghost w-7 h-7 p-0 text-ui-md font-bold inline-flex items-center justify-center"
            onClick={() => {
              void handleMaxDepthStep(-1);
            }}
            disabled={!settings || isUpdatingCore || currentMaxDepth <= MIN_MAX_DEPTH}
            aria-label="Decrease max depth"
          >
            ▼
          </button>
          <div className="min-w-[2ch] text-center text-ui-sm font-bold text-text-strong" aria-live="polite" aria-atomic="true">
            {currentMaxDepth}
          </div>
          <button
            type="button"
            className="ghost w-7 h-7 p-0 text-ui-md font-bold inline-flex items-center justify-center"
            onClick={() => {
              void handleMaxDepthStep(1);
            }}
            disabled={!settings || isUpdatingCore || currentMaxDepth >= MAX_MAX_DEPTH}
            aria-label="Increase max depth"
          >
            ▲
          </button>
        </fieldset>
      </SettingsToggleRow>

      <SettingsSubsection
        title="Create Agent"
        subtitle={
          <>
            Add a custom role under <code>[agents.&lt;name&gt;]</code> and create its config file.
          </>
        }
      />
      <div className="flex flex-col gap-2.5 mb-4.5">
        <div className="flex items-center justify-between gap-2">
          <label className="text-ui-sm font-semibold text-text-strong" htmlFor="settings-agent-create-name">
            Name
          </label>
          <button
            type="button"
            className="ghost w-7 h-7 p-0 inline-flex items-center justify-center rounded-lg"
            onClick={() => {
              if (createDescriptionGenerating || !canGenerateCreateFromName) {
                return;
              }
              void (async () => {
                const generated = await onGenerateCreateDescription({
                  name: createName,
                  description: createDescription,
                  developerInstructions: createDeveloperInstructions,
                });
                if (generated != null) {
                  if (generated.description.trim().length > 0) {
                    setCreateDescription(generated.description);
                  }
                  if (generated.developerInstructions.trim().length > 0) {
                    setCreateDeveloperInstructions(generated.developerInstructions);
                  }
                }
              })();
            }}
            disabled={creatingAgent || createDescriptionGenerating || !canGenerateCreateFromName}
            title="Generate description and developer instructions with AI"
            aria-label="Generate fields for new agent"
          >
            {createDescriptionGenerating ? (
              <MagicSparkleLoaderIcon className="w-3.5 h-3.5 animate-[spin_var(--duration-ui-spinner)_linear_infinite]" />
            ) : (
              <MagicSparkleIcon />
            )}
          </button>
        </div>
        <input
          id="settings-agent-create-name"
          className="flex-1 min-w-0 py-2.5 px-3 rounded-xl border border-border-muted bg-surface-control text-text-strong text-ui-sm outline-none transition-colors focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
          value={createName}
          onChange={(event) => setCreateName(event.target.value)}
          placeholder="researcher"
          disabled={creatingAgent}
        />
        <label className="text-ui-sm font-semibold text-text-strong" htmlFor="settings-agent-create-description">
          Description
        </label>
        <textarea
          id="settings-agent-create-description"
          className="w-full min-h-14 resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-snug px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
          value={createDescription}
          onChange={(event) => setCreateDescription(event.target.value)}
          placeholder="Short role summary."
          rows={2}
          disabled={creatingAgent}
        />
        <label className="text-ui-sm font-semibold text-text-strong" htmlFor="settings-agent-create-developer-instructions">
          Developer instructions
        </label>
        <textarea
          id="settings-agent-create-developer-instructions"
          className="w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
          value={createDeveloperInstructions}
          onChange={(event) => setCreateDeveloperInstructions(event.target.value)}
          placeholder="Multiline per-agent developer instructions."
          disabled={creatingAgent}
        />
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex flex-row items-center gap-1.5 flex-auto min-w-0">
            <span className="text-ui-2xs font-semibold tracking-wide text-text-muted lowercase">model:</span>
            <select
              id="settings-agent-create-model"
              className="py-2 px-2.5 rounded-xl border border-border-muted bg-surface-control text-text-strong text-ui-sm outline-none transition-colors focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
              value={createModel}
              onChange={(event) => setCreateModel(event.target.value)}
              disabled={creatingAgent}
              aria-label="Agent model"
            >
              {effectiveModelOptions.map((option) => (
                <option key={option.model} value={option.model}>
                  {option.model}
                </option>
              ))}
            </select>
          </div>
          <span className="text-ui-xs text-text-muted opacity-80" aria-hidden>
            |
          </span>
          <div className="flex flex-row items-center gap-1.5 flex-auto min-w-0">
            <span className="text-ui-2xs font-semibold tracking-wide text-text-muted lowercase">reasoning:</span>
            <select
              id="settings-agent-create-effort"
              className="py-2 px-2.5 rounded-xl border border-border-muted bg-surface-control text-text-strong text-ui-sm outline-none transition-colors focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
              value={createReasoningEffort}
              onChange={(event) => setCreateReasoningEffort(event.target.value)}
              disabled={creatingAgent || createReasoningOptions.length === 0}
              aria-label="Agent reasoning effort"
            >
              {createReasoningOptions.length === 0 && <option value="">not supported</option>}
              {createReasoningOptions.map((effort) => (
                <option key={effort} value={effort}>
                  {effort}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="inline-flex items-center flex-wrap gap-1.5">
          <button type="button" className="ghost" onClick={() => void handleCreateAgent()}>
            {creatingAgent ? "Creating..." : "Create Agent"}
          </button>
        </div>
        {modelOptions.length === 0 && (
          <div className="text-ui-xs text-text-subtle">
            {modelOptionsLoading
              ? "Loading workspace model metadata. Using fallback model defaults for now."
              : "Using fallback model defaults until workspace model metadata is available."}
          </div>
        )}
        {modelOptionsError && <div className="text-ui-xs text-text-subtle">{modelOptionsError}</div>}
        {createError && <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">{createError}</div>}
      </div>

      <SettingsSubsection
        title="Configured Agents"
        subtitle="Manage custom roles and their per-agent config files."
      />

      {settings && settings.agents.length === 0 && !isLoading && (
        <div className="text-ui-xs text-text-subtle">No custom agents configured yet.</div>
      )}

      {settings?.agents.map((agent) => {
        const isEditing = editingName === agent.name;
        const isPendingDelete = pendingDeleteAgentName === agent.name;
        const isUpdating = updatingAgentName === agent.name;
        const isDeleting = deletingAgentName === agent.name;
        const isReadingConfig = readingConfigAgentName === agent.name;
        const isWritingConfig = writingConfigAgentName === agent.name;
        const isConfigEditorOpen = configEditorAgentName === agent.name;
        const canGenerateEditFromName = editNameDraft.trim().length > 0;
        return (
          <div className="flex flex-col gap-2.5 mb-4.5" key={agent.name}>
            <div className="flex items-start justify-between gap-2.5">
              <div>
                <div className="text-ui-sm font-semibold text-text-strong">{agent.name}</div>
                <div className="text-ui-xs text-text-subtle">
                  {agent.description || "No description."}
                </div>
              </div>
            </div>

            <div className="text-ui-xs text-text-subtle inline mr-1">
              <code>{agent.configFile || "(missing config_file)"}</code>
            </div>
            <div className="inline-flex items-center flex-wrap gap-1.5">
              {!isPendingDelete && (
                <>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => startEditing(agent)}
                    disabled={isUpdating || isDeleting}
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => handleDeleteAgent(agent.name)}
                    disabled={isUpdating || isDeleting}
                  >
                    Delete
                  </button>
                </>
              )}
              <button
                type="button"
                className="ghost"
                onClick={() => void handleOpenPath(agent.resolvedPath)}
              >
                {openInFileManagerLabel()}
              </button>
              <button
                type="button"
                className="ghost"
                onClick={() => void handleOpenConfigEditor(agent.name)}
                disabled={!agent.managedByApp || isReadingConfig || isWritingConfig}
              >
                {isReadingConfig ? "Opening..." : "Edit File"}
              </button>
              {!agent.managedByApp && (
                <span className="text-ui-xs text-text-subtle inline mr-1">External path</span>
              )}
            </div>
            {isPendingDelete && (
              <div className="inline-flex items-center flex-wrap gap-1.5">
                <span className="text-ui-xs text-text-subtle inline mr-1">
                  Delete agent and managed config file?
                </span>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => {
                    setPendingDeleteAgentName(null);
                  }}
                  disabled={isDeleting}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="ghost"
                  onClick={() => void handleConfirmDeleteAgent(agent.name)}
                  disabled={isDeleting}
                >
                  {isDeleting ? "Deleting..." : "Confirm Delete"}
                </button>
              </div>
            )}

            {isEditing && (
              <div className="flex flex-col gap-2.5">
                <div className="flex items-center justify-between gap-2">
                  <label
                    className="text-ui-sm font-semibold text-text-strong"
                    htmlFor={`settings-agent-edit-name-${agent.name}`}
                  >
                    Name
                  </label>
                  <button
                    type="button"
                    className="ghost w-7 h-7 p-0 inline-flex items-center justify-center rounded-lg"
                    onClick={() => {
                      if (editDescriptionGenerating || !canGenerateEditFromName) {
                        return;
                      }
                      void (async () => {
                        const generated = await onGenerateEditDescription({
                          name: editNameDraft || agent.name,
                          description: editDescriptionDraft,
                          developerInstructions: editDeveloperInstructionsDraft,
                        });
                        if (generated != null) {
                          if (generated.description.trim().length > 0) {
                            setEditDescriptionDraft(generated.description);
                          }
                          if (generated.developerInstructions.trim().length > 0) {
                            setEditDeveloperInstructionsDraft(generated.developerInstructions);
                          }
                        }
                      })();
                    }}
                    disabled={isUpdating || editDescriptionGenerating || !canGenerateEditFromName}
                    title="Generate description and developer instructions with AI"
                    aria-label={`Generate fields for ${agent.name}`}
                  >
                    {editDescriptionGenerating ? (
                      <MagicSparkleLoaderIcon className="w-3.5 h-3.5 animate-[spin_var(--duration-ui-spinner)_linear_infinite]" />
                    ) : (
                      <MagicSparkleIcon />
                    )}
                  </button>
                </div>
                <input
                  id={`settings-agent-edit-name-${agent.name}`}
                  className="flex-1 min-w-0 py-2.5 px-3 rounded-xl border border-border-muted bg-surface-control text-text-strong text-ui-sm outline-none transition-colors focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
                  value={editNameDraft}
                  onChange={(event) => setEditNameDraft(event.target.value)}
                  disabled={isUpdating}
                />
                <label
                  className="text-ui-sm font-semibold text-text-strong"
                  htmlFor={`settings-agent-edit-description-${agent.name}`}
                >
                  Description
                </label>
                <textarea
                  id={`settings-agent-edit-description-${agent.name}`}
                  className="w-full min-h-14 resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-snug px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
                  value={editDescriptionDraft}
                  onChange={(event) => setEditDescriptionDraft(event.target.value)}
                  placeholder="Short role summary."
                  rows={2}
                  disabled={isUpdating}
                />
                <label
                  className="text-ui-sm font-semibold text-text-strong"
                  htmlFor={`settings-agent-edit-developer-instructions-${agent.name}`}
                >
                  Developer instructions
                </label>
                <textarea
                  id={`settings-agent-edit-developer-instructions-${agent.name}`}
                  className="w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
                  value={editDeveloperInstructionsDraft}
                  onChange={(event) => setEditDeveloperInstructionsDraft(event.target.value)}
                  placeholder="Multiline per-agent developer instructions."
                  disabled={isUpdating}
                />
                <label className="inline-flex items-center gap-2 text-ui-sm text-text-subtle">
                  <input
                    type="checkbox"
                    checked={renameManagedFile}
                    onChange={(event) => setRenameManagedFile(event.target.checked)}
                  />
                  Rename managed config file when agent name changes
                </label>
                <div className="inline-flex items-center flex-wrap gap-1.5">
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => {
                      setEditingName(null);
                      setEditError(null);
                    }}
                    disabled={isUpdating}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => void handleUpdateAgent()}
                    disabled={isUpdating}
                  >
                    {isUpdating ? "Saving..." : "Save"}
                  </button>
                </div>
                {editError && <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">{editError}</div>}
              </div>
            )}

            {isConfigEditorOpen && (
              <div className="flex flex-col gap-2.5 mb-4.5">
                <div className="flex items-center justify-between gap-2.5">
                  <div>
                    <div className="text-ui-sm font-semibold text-text-strong">{agent.name} config file</div>
                    <div className="text-ui-xs text-text-subtle">
                      <code>{agent.configFile}</code>
                    </div>
                  </div>
                  <div className="inline-flex items-center flex-wrap gap-1.5">
                    <button
                      type="button"
                      className="ghost"
                      onClick={() => {
                        setConfigEditorAgentName(null);
                        setConfigEditorDirty(false);
                      }}
                    >
                      Close
                    </button>
                    <button
                      type="button"
                      className="ghost"
                      onClick={() => void handleSaveConfigEditor()}
                      disabled={!configEditorDirty || writingConfigAgentName === agent.name}
                    >
                      {isWritingConfig ? "Saving..." : "Save"}
                    </button>
                  </div>
                </div>
                <textarea
                  className="w-full min-h-[150px] resize-y rounded-xl border border-border-muted bg-surface-1 text-text-strong font-code text-ui-sm leading-relaxed px-3 py-2.5 outline-none focus:border-border-strong focus:shadow-[0_0_0_3px_rgba(99,102,241,0.16)]"
                  value={configEditorContent}
                  onChange={(event) => {
                    setConfigEditorContent(event.target.value);
                    setConfigEditorDirty(true);
                  }}
                />
              </div>
            )}
          </div>
        );
      })}

      {isLoading && <div className="text-ui-xs text-text-subtle">Loading agents settings...</div>}
      {openPathError && <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">{openPathError}</div>}
      {error && <div className="text-ui-sm text-status-error bg-[rgba(236,72,153,0.08)] rounded-xl px-2.5 py-2 border border-[rgba(236,72,153,0.2)]">{error}</div>}
    </SettingsSection>
  );
}
