import { SettingsCard } from "@shared/composites";
import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { Download, HardDrive, Plus, Trash2, Volume2, X } from "lucide-react";
import { useState } from "react";

// ── Types ────────────────────────────────────────────────────────────

interface VoiceConfig {
  enabled?: boolean;
  input?: InputConfig;
  output?: OutputConfig;
  conversation?: ConversationConfig;
}

interface InputConfig {
  hotkey?: string;
  silenceThresholdSecs?: number;
  privacyMode?: string;
  sttEngine?: string;
  vadThreshold?: number;
  useNeuralVad?: boolean;
  allowedLanguages?: string[];
}

interface OutputConfig {
  enabled?: boolean;
  speakingRate?: number;
  speakDuringFocus?: boolean;
  ttsEngine?: string;
  defaultPersona?: string;
  personas?: Record<string, PersonaConfig>;
}

interface PersonaConfig {
  type: "preset" | "custom";
  speaker?: string;
  description?: string;
  speed?: number;
  temperature?: number;
}

interface ConversationConfig {
  warmSessionMinutes?: number;
  warmChatMinutes?: number;
  silenceThresholdSecs?: number;
  autoResume?: boolean;
  adaptiveBreath?: boolean;
}

interface AudioDevices {
  input: string[];
  output: string[];
  defaultInput: string | null;
  defaultOutput: string | null;
}

interface ModelStatus {
  downloaded: boolean;
  displayName: string;
  dirName: string;
}

interface ModelStatusResponse {
  asr: ModelStatus;
  tts: ModelStatus;
  ttsInstruct: ModelStatus;
  sttLoaded: boolean;
  ttsLoaded: boolean;
}

// ── Constants ────────────────────────────────────────────────────────

const PRIVACY_MODES = [
  { value: "standard", label: "Standard", desc: "Full cognitive pipeline" },
  { value: "strict", label: "Strict", desc: "Skip mirror lookups, pronunciation history" },
  { value: "off", label: "Off", desc: "No privacy restrictions" },
] as const;

const ASR_LANGUAGES = [
  { code: "en", label: "English" },
  { code: "zh", label: "Chinese" },
  { code: "vi", label: "Vietnamese" },
  { code: "ja", label: "Japanese" },
  { code: "ko", label: "Korean" },
  { code: "fr", label: "French" },
  { code: "de", label: "German" },
  { code: "es", label: "Spanish" },
] as const;

const PERSONA_LABELS: Record<string, string> = {
  neutral: "Neutral (Serena)",
  professional: "Professional (Ryan)",
  friendly: "Friendly (Vivian)",
  calm: "Calm (Aiden)",
  energetic: "Energetic (Dylan)",
  storyteller: "Storyteller (Eric)",
};

// ── Persona list with custom voice creation ──────────────────────

function PersonaList({
  personaKeys,
  personas,
  defaultPersona,
  ttsInstructReady,
  onSelect,
  onDelete,
  onTest,
  onCreateCustom,
}: {
  personaKeys: string[];
  personas: Record<string, PersonaConfig>;
  defaultPersona: string;
  ttsInstructReady: boolean;
  onSelect: (key: string) => Promise<void>;
  onDelete: (key: string) => Promise<void>;
  onTest: (key: string) => Promise<void>;
  onCreateCustom: (
    name: string,
    description: string,
    speed: number,
    temperature: number,
  ) => Promise<void>;
}) {
  const [testing, setTesting] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [newSpeed, setNewSpeed] = useState("1.0");
  const [newTemp, setNewTemp] = useState("0.9");
  const [creating, setCreating] = useState(false);

  const handleCreate = async () => {
    const name = newName.trim().toLowerCase().replace(/\s+/g, "-");
    if (!name || !newDesc.trim()) return;
    setCreating(true);
    await onCreateCustom(
      name,
      newDesc.trim(),
      Number.parseFloat(newSpeed) || 1.0,
      Number.parseFloat(newTemp) || 0.9,
    );
    setCreating(false);
    setShowCreate(false);
    setNewName("");
    setNewDesc("");
    setNewSpeed("1.0");
    setNewTemp("0.9");
  };

  return (
    <div>
      {!ttsInstructReady && (
        <p className="text-ui-xs text-fg-secondary mb-2">
          Download the 1.7B CustomVoice model above to unlock voice personas.
        </p>
      )}
      <div
        className={`rounded-lg border border-separator/50 divide-y divide-separator/30 ${!ttsInstructReady ? "opacity-50 pointer-events-none" : ""}`}
      >
        {personaKeys.map((key) => {
          const p = personas[key];
          const isActive = key === defaultPersona && ttsInstructReady;
          const isCustom = p.type === "custom";
          const isBuiltIn = PERSONA_LABELS[key] !== undefined;
          const detail =
            p.type === "preset"
              ? `${p.speaker} · ${(p.speed ?? 1).toFixed(1)}x`
              : `${(p.description ?? "").slice(0, 30)} · ${(p.speed ?? 1).toFixed(1)}x`;

          return (
            <div
              key={key}
              className={`flex items-center gap-2 px-3 py-1.5 transition-colors ${
                isActive ? "bg-brand/5" : "hover:bg-control-hover/40"
              }`}
            >
              <button
                type="button"
                onClick={() => {
                  if (isActive) return;
                  onSelect(key);
                }}
                className="flex-1 min-w-0 flex items-center gap-2 text-left"
              >
                <span className="text-ui-sm font-medium text-fg truncate">
                  {PERSONA_LABELS[key] ?? key}
                </span>
                {isActive && (
                  <span className="text-[9px] text-brand font-medium uppercase tracking-wider shrink-0">
                    Active
                  </span>
                )}
                {isCustom && (
                  <span className="text-[9px] text-purple-400/70 font-medium shrink-0">Custom</span>
                )}
                <span className="text-[10px] text-fg-dim ml-auto shrink-0">{detail}</span>
              </button>
              <button
                type="button"
                onClick={async (e) => {
                  e.stopPropagation();
                  setTesting(key);
                  await onTest(key);
                  setTimeout(() => setTesting(null), 3000);
                }}
                disabled={testing === key}
                className={`size-5 flex items-center justify-center rounded transition-colors shrink-0 ${
                  testing === key
                    ? "text-brand animate-pulse"
                    : "text-fg-secondary hover:text-brand"
                }`}
                title="Preview voice"
              >
                <Volume2 className="size-3" strokeWidth={1.5} />
              </button>
              {isCustom && !isBuiltIn && !isActive && (
                <button
                  type="button"
                  onClick={() => onDelete(key)}
                  className="size-5 flex items-center justify-center text-fg-secondary hover:text-red-400 rounded transition-colors shrink-0"
                  title="Delete"
                >
                  <Trash2 className="size-2.5" strokeWidth={1.5} />
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Create custom voice */}
      {showCreate ? (
        <div className="px-3 py-3 rounded-lg border border-purple-400/30 bg-purple-400/5 space-y-2.5">
          <div className="flex items-center justify-between">
            <span className="text-ui-sm font-medium text-purple-300">New Custom Voice</span>
            <button
              type="button"
              onClick={() => setShowCreate(false)}
              className="size-5 flex items-center justify-center text-fg-secondary hover:text-fg rounded transition-colors"
            >
              <X className="size-3.5" strokeWidth={1.5} />
            </button>
          </div>
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Name (e.g. narrator)"
            className="w-full px-2.5 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors placeholder:text-fg-dim"
          />
          <textarea
            value={newDesc}
            onChange={(e) => setNewDesc(e.target.value)}
            placeholder="Describe the voice (e.g. &quot;Calm female therapist, warm and empathetic, mid-30s&quot;)"
            rows={2}
            className="w-full px-2.5 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors placeholder:text-fg-dim resize-none"
          />
          <div className="flex gap-2">
            <label className="flex-1">
              <span className="block text-[10px] text-fg-dim mb-0.5">Speed</span>
              <input
                type="number"
                value={newSpeed}
                onChange={(e) => setNewSpeed(e.target.value)}
                step="0.05"
                min="0.5"
                max="2.0"
                className="w-full px-2.5 py-1 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              />
            </label>
            <label className="flex-1">
              <span className="block text-[10px] text-fg-dim mb-0.5">Temperature</span>
              <input
                type="number"
                value={newTemp}
                onChange={(e) => setNewTemp(e.target.value)}
                step="0.05"
                min="0.1"
                max="1.0"
                className="w-full px-2.5 py-1 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              />
            </label>
          </div>
          {!ttsInstructReady && (
            <p className="text-[10px] text-status-warning/70">
              The 1.7B CustomVoice model will be downloaded automatically when you save.
            </p>
          )}
          <button
            type="button"
            onClick={handleCreate}
            disabled={!newName.trim() || !newDesc.trim() || creating}
            className="w-full py-1.5 text-ui-sm font-medium rounded-md bg-purple-500/20 text-purple-300 hover:bg-purple-500/30 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            {creating ? "Creating..." : "Create Voice"}
          </button>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 w-full px-3 py-2 rounded-lg border border-dashed border-separator/50 text-fg-secondary hover:border-purple-400/30 hover:text-purple-300 transition-colors"
        >
          <Plus className="size-3.5" strokeWidth={1.5} />
          <span className="text-ui-xs">Create custom voice</span>
          <span className="text-[9px] text-fg-dim ml-auto">Requires 1.7B model</span>
        </button>
      )}
    </div>
  );
}

// ── Component ────────────────────────────────────────────────────────

export function VoiceSettings() {
  const toast = useToastContext();

  const { data: voice, refetch } = useQuery<VoiceConfig>(
    "config_get_section",
    { section: "voice" },
    { enabled: true, input: {}, output: {}, conversation: {} },
  );

  const { data: devices } = useQuery<AudioDevices>("voice_list_devices", undefined, {
    input: [],
    output: [],
    defaultInput: null,
    defaultOutput: null,
  });

  const { data: models, refetch: refetchModels } = useQuery<ModelStatusResponse>(
    "voice_model_status",
    undefined,
    {
      asr: { downloaded: false, displayName: "...", dirName: "" },
      tts: { downloaded: false, displayName: "...", dirName: "" },
      ttsInstruct: { downloaded: false, displayName: "...", dirName: "" },
      sttLoaded: false,
      ttsLoaded: false,
    },
  );

  // Listen for download progress events
  const [downloadProgress, setDownloadProgress] = useState<Record<string, number>>({});

  useEvent<{ model: string; progress: number }>("voice:model_progress", (payload) => {
    setDownloadProgress((prev) => ({ ...prev, [payload.model]: payload.progress }));
    if (payload.progress >= 1.0) {
      // Refresh model status after download completes
      setTimeout(() => {
        refetchModels();
        setDownloadProgress((prev) => {
          const next = { ...prev };
          delete next[payload.model];
          return next;
        });
      }, 500);
    }
  });

  // ── Input settings state ─────────────────────────────────────────

  const [inputEdits, setInputEdits] = useState<Record<string, unknown>>({});
  const [savingInput, setSavingInput] = useState(false);

  const inputVal = <T,>(key: string, fallback: T): T => {
    if (key in inputEdits) return inputEdits[key] as T;
    const val = (voice.input as Record<string, unknown> | undefined)?.[key];
    return (val !== undefined ? val : fallback) as T;
  };

  const hasInputChanges = Object.keys(inputEdits).length > 0;

  const saveInput = async () => {
    setSavingInput(true);
    try {
      await ipc("config_update_section", { section: "voice", patch: { input: inputEdits } });
      refetch();
      setInputEdits({});
    } catch {
      toast.show("Failed to save input settings");
    } finally {
      setSavingInput(false);
    }
  };

  // ── Output settings state ────────────────────────────────────────

  const [outputEdits, setOutputEdits] = useState<Record<string, unknown>>({});
  const [savingOutput, setSavingOutput] = useState(false);

  const outputVal = <T,>(key: string, fallback: T): T => {
    if (key in outputEdits) return outputEdits[key] as T;
    const val = (voice.output as Record<string, unknown> | undefined)?.[key];
    return (val !== undefined ? val : fallback) as T;
  };

  const hasOutputChanges = Object.keys(outputEdits).length > 0;

  const saveOutput = async () => {
    setSavingOutput(true);
    try {
      await ipc("config_update_section", { section: "voice", patch: { output: outputEdits } });
      refetch();
      setOutputEdits({});
    } catch {
      toast.show("Failed to save output settings");
    } finally {
      setSavingOutput(false);
    }
  };

  // ── Conversation settings state ──────────────────────────────────

  const [convEdits, setConvEdits] = useState<Record<string, unknown>>({});
  const [savingConv, setSavingConv] = useState(false);

  const convVal = <T,>(key: string, fallback: T): T => {
    if (key in convEdits) return convEdits[key] as T;
    const val = (voice.conversation as Record<string, unknown> | undefined)?.[key];
    return (val !== undefined ? val : fallback) as T;
  };

  const hasConvChanges = Object.keys(convEdits).length > 0;

  const saveConv = async () => {
    setSavingConv(true);
    try {
      await ipc("config_update_section", {
        section: "voice",
        patch: { conversation: convEdits },
      });
      refetch();
      setConvEdits({});
    } catch {
      toast.show("Failed to save conversation settings");
    } finally {
      setSavingConv(false);
    }
  };

  // ── Model actions ────────────────────────────────────────────────

  const [modelActing, setModelActing] = useState<string | null>(null);

  const handleDownload = async (modelKey: string) => {
    setModelActing(modelKey);
    try {
      await ipc("voice_download_model", { model: modelKey });
    } catch {
      toast.show("Failed to start download");
    } finally {
      setModelActing(null);
    }
  };

  const handleDelete = async (modelKey: string) => {
    setModelActing(modelKey);
    try {
      await ipc("voice_delete_model", { model: modelKey });
      refetchModels();
    } catch {
      toast.show("Failed to delete model");
    } finally {
      setModelActing(null);
    }
  };

  // ── Personas ─────────────────────────────────────────────────────

  const personas = voice.output?.personas ?? {};
  const personaKeys = Object.keys(personas);
  const defaultPersona = outputVal("defaultPersona", "neutral");

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-fg">Voice</h2>
        <p className="text-ui text-fg-secondary mt-1">
          Audio devices, speech recognition, TTS, and voice models
        </p>
      </div>

      <div className="space-y-4">
        {/* ── Audio Devices ─────────────────────────────────── */}
        <SettingsCard title="Audio Devices">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Microphone</span>
              <select
                value={inputVal("selectedDevice", "")}
                onChange={(e) =>
                  setInputEdits((prev) => ({
                    ...prev,
                    selectedDevice: e.target.value || null,
                  }))
                }
                className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              >
                <option value="" className="bg-glass-strong">
                  System default{devices.defaultInput ? ` (${devices.defaultInput})` : ""}
                </option>
                {devices.input.map((d) => (
                  <option key={d} value={d} className="bg-glass-strong">
                    {d}
                  </option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Speaker</span>
              <select
                value={outputVal("selectedDevice", "")}
                onChange={(e) =>
                  setOutputEdits((prev) => ({
                    ...prev,
                    selectedDevice: e.target.value || null,
                  }))
                }
                className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              >
                <option value="" className="bg-glass-strong">
                  System default{devices.defaultOutput ? ` (${devices.defaultOutput})` : ""}
                </option>
                {devices.output.map((d) => (
                  <option key={d} value={d} className="bg-glass-strong">
                    {d}
                  </option>
                ))}
              </select>
            </label>

            {(hasInputChanges || hasOutputChanges) && (
              <>
                <p className="text-[10px] text-fg-dim">
                  Device changes apply on next voice session.
                </p>
                <SaveButton
                  onClick={async () => {
                    if (hasInputChanges) await saveInput();
                    if (hasOutputChanges) await saveOutput();
                  }}
                  saving={savingInput || savingOutput}
                />
              </>
            )}
          </div>
        </SettingsCard>

        {/* ── Speech Recognition ────────────────────────────── */}
        <SettingsCard title="Speech Recognition">
          <div className="space-y-3">
            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-ui-xs text-fg-secondary mb-1">
                  Silence threshold (seconds)
                </span>
                <input
                  type="number"
                  value={inputVal("silenceThresholdSecs", 1.5)}
                  onChange={(e) =>
                    setInputEdits((prev) => ({
                      ...prev,
                      silenceThresholdSecs: Number.parseFloat(e.target.value) || 1.5,
                    }))
                  }
                  step="0.1"
                  min="0.5"
                  max="5"
                  className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-ui-xs text-fg-secondary mb-1">
                  VAD sensitivity (0-1)
                </span>
                <input
                  type="number"
                  value={inputVal("vadThreshold", 0.5)}
                  onChange={(e) =>
                    setInputEdits((prev) => ({
                      ...prev,
                      vadThreshold: Number.parseFloat(e.target.value) || 0.5,
                    }))
                  }
                  step="0.05"
                  min="0"
                  max="1"
                  className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
                />
              </label>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Noise reduction</span>
                <p className="text-ui-xs text-fg-dim">
                  RNNoise filter — helps in noisy environments. Disable for high-quality mics.
                </p>
              </div>
              <Toggle
                checked={inputVal("noiseReduction", false)}
                onChange={(v) => setInputEdits((prev) => ({ ...prev, noiseReduction: v }))}
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Neural VAD</span>
                <p className="text-ui-xs text-fg-dim">Use WebRTC VAD instead of RMS threshold</p>
              </div>
              <Toggle
                checked={inputVal("useNeuralVad", false)}
                onChange={(v) => setInputEdits((prev) => ({ ...prev, useNeuralVad: v }))}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Privacy mode</span>
              <select
                value={inputVal("privacyMode", "standard")}
                onChange={(e) =>
                  setInputEdits((prev) => ({ ...prev, privacyMode: e.target.value }))
                }
                className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              >
                {PRIVACY_MODES.map((m) => (
                  <option key={m.value} value={m.value} className="bg-glass-strong">
                    {m.label} — {m.desc}
                  </option>
                ))}
              </select>
            </label>

            <div>
              <span className="block text-ui-xs text-fg-secondary mb-2">Allowed languages</span>
              <div className="flex flex-wrap gap-2">
                {(() => {
                  const selected = inputVal<string[]>("allowedLanguages", ["en", "zh", "vi"]);
                  return ASR_LANGUAGES.map((lang) => {
                    const isChecked = selected.includes(lang.code);
                    return (
                      <button
                        key={lang.code}
                        type="button"
                        onClick={() => {
                          const next = isChecked
                            ? selected.filter((c) => c !== lang.code)
                            : [...selected, lang.code];
                          setInputEdits((prev) => ({ ...prev, allowedLanguages: next }));
                        }}
                        className={`px-2.5 py-1 rounded-md text-ui-xs font-medium border transition-colors ${
                          isChecked
                            ? "border-brand/40 bg-brand/10 text-brand"
                            : "border-separator/50 bg-control-hover/30 text-fg-secondary hover:border-separator hover:text-fg"
                        }`}
                      >
                        {lang.label}
                      </button>
                    );
                  });
                })()}
              </div>
              <p className="text-[10px] text-fg-dim mt-1.5">
                Restricts ASR to these languages. At least one must be selected.
              </p>
            </div>

            {hasInputChanges && (
              <>
                <p className="text-ui-xs text-fg-dim">Changes apply on next voice session</p>
                <SaveButton onClick={saveInput} saving={savingInput} />
              </>
            )}
          </div>
        </SettingsCard>

        {/* ── Text-to-Speech ────────────────────────────────── */}
        <SettingsCard title="Text-to-Speech">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Enable TTS</span>
                <p className="text-ui-xs text-fg-dim">Speak agent responses aloud</p>
              </div>
              <Toggle
                checked={outputVal("enabled", true)}
                onChange={(v) => setOutputEdits((prev) => ({ ...prev, enabled: v }))}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">TTS Engine</span>
              <select
                value={outputVal("ttsEngine", "qwen3")}
                onChange={(e) => setOutputEdits((prev) => ({ ...prev, ttsEngine: e.target.value }))}
                className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              >
                {models.ttsInstruct.downloaded && (
                  <option value="qwen3" className="bg-glass-strong">
                    {models.ttsInstruct.displayName} (voice personas)
                  </option>
                )}
                {models.tts.downloaded && (
                  <option value="qwen3Base" className="bg-glass-strong">
                    {models.tts.displayName} (single voice)
                  </option>
                )}
                {!models.tts.downloaded && !models.ttsInstruct.downloaded && (
                  <option value="qwen3" className="bg-glass-strong">
                    Qwen3-TTS — download a model below
                  </option>
                )}
                <option value="system" className="bg-glass-strong">
                  System (macOS AVSpeech)
                </option>
              </select>
            </label>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Speaking rate</span>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  value={outputVal("speakingRate", 1.0)}
                  onChange={(e) =>
                    setOutputEdits((prev) => ({
                      ...prev,
                      speakingRate: Number.parseFloat(e.target.value),
                    }))
                  }
                  min="0.5"
                  max="2.0"
                  step="0.05"
                  className="flex-1 accent-brand"
                />
                <span className="text-ui-xs text-fg-secondary w-10 text-right">
                  {outputVal("speakingRate", 1.0).toFixed(2)}x
                </span>
              </div>
            </label>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Speak during focus</span>
                <p className="text-ui-xs text-fg-dim">Allow TTS while user is typing</p>
              </div>
              <Toggle
                checked={outputVal("speakDuringFocus", false)}
                onChange={(v) => setOutputEdits((prev) => ({ ...prev, speakDuringFocus: v }))}
              />
            </div>

            {hasOutputChanges && (
              <>
                <p className="text-ui-xs text-fg-dim">Changes apply on next voice session</p>
                <SaveButton onClick={saveOutput} saving={savingOutput} />
              </>
            )}
          </div>
        </SettingsCard>

        {/* ── AI Models ─────────────────────────────────────── */}
        <SettingsCard title="AI Models">
          <div className="space-y-2">
            <div className="flex items-center gap-3 -mt-1 mb-2">
              <p className="text-ui-xs text-fg-dim flex-1">
                On-device models for speech recognition and synthesis. Downloaded from HuggingFace.
              </p>
              <div className="flex items-center gap-3 shrink-0">
                <span
                  className={`text-[10px] font-medium ${models.sttLoaded ? "text-green-400" : "text-fg-secondary"}`}
                >
                  STT: {models.sttLoaded ? "Active" : "Off"}
                </span>
                <span
                  className={`text-[10px] font-medium ${models.ttsLoaded ? "text-green-400" : "text-fg-secondary"}`}
                >
                  TTS: {models.ttsLoaded ? "Active" : "Off"}
                </span>
              </div>
            </div>

            {(
              [
                ["asr", models.asr, "Speech recognition (required for voice input)"],
                ["tts", models.tts, "Text-to-speech base (0.6B, single voice fallback)"],
                [
                  "ttsInstruct",
                  models.ttsInstruct,
                  "Text-to-speech (1.7B, required for voice personas)",
                ],
              ] as const
            ).map(([key, status, desc]) => {
              const progress = downloadProgress[status.displayName];
              const isDownloading = progress !== undefined && progress < 1.0;
              return (
                <div
                  key={key}
                  className="flex items-center gap-3 px-3 py-2.5 rounded-lg border border-separator/50 bg-control-hover/30"
                >
                  <HardDrive
                    className={`size-4 shrink-0 ${status.downloaded ? "text-green-400" : "text-fg-secondary"}`}
                    strokeWidth={1.5}
                  />
                  <div className="flex-1 min-w-0">
                    <div className="text-ui-sm font-medium text-fg">{status.displayName}</div>
                    <div className="text-[10px] text-fg-dim">{desc}</div>
                    {isDownloading && (
                      <div className="mt-1.5 h-1 bg-control-hover rounded-full overflow-hidden">
                        <div
                          className="h-full bg-brand rounded-full transition-all duration-300"
                          style={{ width: `${Math.round(progress * 100)}%` }}
                        />
                      </div>
                    )}
                  </div>
                  <div className="flex items-center gap-1.5 shrink-0">
                    {status.downloaded ? (
                      <>
                        <span className="text-[10px] text-green-400 font-medium mr-1">Ready</span>
                        <button
                          type="button"
                          onClick={() => handleDelete(key)}
                          disabled={modelActing === key}
                          className="size-7 flex items-center justify-center text-fg-secondary hover:text-red-400 rounded-md hover:bg-red-400/10 transition-colors"
                          title="Delete model"
                        >
                          <Trash2 className="size-3.5" strokeWidth={1.5} />
                        </button>
                      </>
                    ) : isDownloading ? (
                      <span className="text-[10px] text-brand font-medium">
                        {Math.round(progress * 100)}%
                      </span>
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleDownload(key)}
                        disabled={modelActing === key}
                        className="size-7 flex items-center justify-center text-fg-secondary hover:text-brand rounded-md hover:bg-brand/10 transition-colors"
                        title="Download model"
                      >
                        <Download className="size-3.5" strokeWidth={1.5} />
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </SettingsCard>

        {/* ── Voice Persona ─────────────────────────────────── */}
        <SettingsCard title="Voice Persona">
          <div className="space-y-3">
            {(() => {
              const engine = outputVal<string>("ttsEngine", "qwen3");
              const personasDisabledReason =
                engine === "system"
                  ? "Voice personas are not available with system TTS."
                  : engine === "qwen3Base"
                    ? "Voice personas require the 1.7B CustomVoice model. Switch to it above."
                    : !models.ttsInstruct.downloaded
                      ? "Download the 1.7B CustomVoice model above to unlock voice personas."
                      : null;
              if (personasDisabledReason) {
                return <p className="text-ui-xs text-fg-secondary">{personasDisabledReason}</p>;
              }
              return null;
            })()}
            <PersonaList
              personaKeys={personaKeys}
              personas={personas}
              defaultPersona={defaultPersona}
              ttsInstructReady={
                outputVal("ttsEngine", "qwen3") === "qwen3" && models.ttsInstruct.downloaded
              }
              onSelect={async (key) => {
                try {
                  await ipc("config_update_section", {
                    section: "voice",
                    patch: { output: { defaultPersona: key } },
                  });
                  refetch();
                } catch {
                  toast.show("Failed to switch persona");
                }
              }}
              onTest={async (key) => {
                try {
                  await ipc("voice_test_persona", { persona: key });
                } catch {
                  toast.show("Failed to preview voice");
                }
              }}
              onDelete={async (key) => {
                const patch: Record<string, unknown> = { [key]: null };
                const resetDefault = defaultPersona === key ? { defaultPersona: "neutral" } : {};
                try {
                  await ipc("config_update_section", {
                    section: "voice",
                    patch: { output: { personas: patch, ...resetDefault } },
                  });
                  refetch();
                } catch {
                  toast.show("Failed to delete persona");
                }
              }}
              onCreateCustom={async (name, description, speed, temperature) => {
                const persona = { type: "custom", description, speed, temperature };
                try {
                  await ipc("config_update_section", {
                    section: "voice",
                    patch: { output: { personas: { [name]: persona } } },
                  });
                  refetch();
                  if (!models.ttsInstruct.downloaded) {
                    await ipc("voice_download_model", { model: "ttsInstruct" });
                  }
                } catch {
                  toast.show("Failed to create persona");
                }
              }}
            />
          </div>
        </SettingsCard>

        {/* ── Conversation ──────────────────────────────────── */}
        <SettingsCard title="Conversation">
          <div className="space-y-3">
            <div className="flex gap-3">
              <label className="flex-1">
                <span className="block text-ui-xs text-fg-secondary mb-1">
                  Warm session (minutes)
                </span>
                <input
                  type="number"
                  value={convVal("warmSessionMinutes", 15)}
                  onChange={(e) =>
                    setConvEdits((prev) => ({
                      ...prev,
                      warmSessionMinutes: Number.parseInt(e.target.value, 10) || 15,
                    }))
                  }
                  min="1"
                  max="120"
                  className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
                />
              </label>
              <label className="flex-1">
                <span className="block text-ui-xs text-fg-secondary mb-1">Warm chat (minutes)</span>
                <input
                  type="number"
                  value={convVal("warmChatMinutes", 5)}
                  onChange={(e) =>
                    setConvEdits((prev) => ({
                      ...prev,
                      warmChatMinutes: Number.parseInt(e.target.value, 10) || 5,
                    }))
                  }
                  min="1"
                  max="60"
                  className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
                />
              </label>
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">
                Turn silence (seconds)
              </span>
              <input
                type="number"
                value={convVal("silenceThresholdSecs", 1.5)}
                onChange={(e) =>
                  setConvEdits((prev) => ({
                    ...prev,
                    silenceThresholdSecs: Number.parseFloat(e.target.value) || 1.5,
                  }))
                }
                step="0.1"
                min="0.5"
                max="5"
                className="w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors"
              />
              <p className="text-[10px] text-fg-dim mt-1">
                Silence duration to end a voice conversation turn
              </p>
            </label>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Auto-resume</span>
                <p className="text-ui-xs text-fg-dim">Resume listening after agent response</p>
              </div>
              <Toggle
                checked={convVal("autoResume", true)}
                onChange={(v) => setConvEdits((prev) => ({ ...prev, autoResume: v }))}
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Adaptive breath</span>
                <p className="text-ui-xs text-fg-dim">Variable pause based on response length</p>
              </div>
              <Toggle
                checked={convVal("adaptiveBreath", true)}
                onChange={(v) => setConvEdits((prev) => ({ ...prev, adaptiveBreath: v }))}
              />
            </div>

            {hasConvChanges && (
              <>
                <p className="text-ui-xs text-fg-dim">Changes apply on next voice session</p>
                <SaveButton onClick={saveConv} saving={savingConv} />
              </>
            )}
          </div>
        </SettingsCard>
      </div>
    </div>
  );
}
