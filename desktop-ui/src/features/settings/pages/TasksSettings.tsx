import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { useState } from "react";

// ── Types ────────────────────────────────────────────────────────────

interface TodoData {
  focus?: { maxSlots?: number; deadlineHours?: number };
  notifications?: {
    targets?: string[];
    focusReminders?: boolean;
    dailyDigest?: boolean;
    dailyDigestTime?: string;
  };
  enrichment?: {
    enabled?: boolean;
    autoApplyThreshold?: number;
    useLlm?: boolean;
  };
  search?: {
    enabled?: boolean;
    semanticThreshold?: number;
    embeddingModel?: string;
    rrfK?: number;
  };
  dailyPlanning?: { enabled?: boolean; planningTime?: string };
}

type Section = keyof TodoData;

const NOTIFICATION_TARGET_OPTIONS = [
  { value: "os_native", label: "System notifications" },
  { value: "telegram", label: "Telegram" },
  { value: "discord", label: "Discord" },
  { value: "slack", label: "Slack" },
  { value: "email", label: "Email" },
];

const DEFAULT_TARGETS: string[] = ["os_native"];

const INPUT_CLASS =
  "w-full px-3 py-1.5 text-ui-sm text-fg bg-control-hover border border-separator rounded-control focus:outline-none focus:border-fg-secondary/50 focus:ring-2 focus:ring-separator transition-colors placeholder:text-fg-dim";

// ── Component ────────────────────────────────────────────────────────

export function TasksSettings() {
  const toast = useToastContext();
  const { data: todo, refetch } = useQuery<TodoData>("config_get_section", { section: "todo" }, {});

  const [edits, setEdits] = useState<Partial<Record<Section, Record<string, unknown>>>>({});
  const [saving, setSaving] = useState<Section | null>(null);

  // ── Helpers ──────────────────────────────────────────────────────

  const setEdit = (section: Section, key: string, value: unknown) => {
    setEdits((prev) => ({ ...prev, [section]: { ...prev[section], [key]: value } }));
  };

  const val = <T,>(section: Section, key: string, fallback: T): T => {
    const sectionEdits = edits[section];
    if (sectionEdits && key in sectionEdits) return sectionEdits[key] as T;
    const data = todo[section] as Record<string, unknown> | undefined;
    return (data?.[key] as T) ?? fallback;
  };

  const isDirty = (section: Section) => Object.keys(edits[section] ?? {}).length > 0;

  const save = async (section: Section) => {
    const sectionEdits = edits[section];
    if (!sectionEdits || Object.keys(sectionEdits).length === 0) return;
    setSaving(section);
    try {
      await ipc("config_update_section", {
        section: "todo",
        patch: { [section]: sectionEdits },
      });
      setEdits((prev) => {
        const next = { ...prev };
        delete next[section];
        return next;
      });
      refetch();
    } catch {
      toast.show("Failed to save settings");
    } finally {
      setSaving(null);
    }
  };

  // ── Notification targets ────────────────────────────────────────

  const currentTargets: string[] = val("notifications", "targets", DEFAULT_TARGETS);

  const toggleTarget = (target: string) => {
    const next = currentTargets.includes(target)
      ? currentTargets.filter((t) => t !== target)
      : [...currentTargets, target];
    setEdit("notifications", "targets", next);
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-fg">Tasks & Notifications</h2>
        <p className="text-ui text-fg-secondary mt-1">
          Focus mode, notifications, enrichment, and planning settings
        </p>
      </div>

      <div className="space-y-4">
        {/* ── Focus ──────────────────────────────────────────────── */}
        <SettingsCard title="Focus mode">
          <div className="space-y-3">
            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">Maximum focus slots</span>
              <p className="text-ui-xs text-fg-dim mb-1">
                How many tasks can be in focus simultaneously
              </p>
              <input
                type="number"
                min={1}
                max={10}
                value={val("focus", "maxSlots", 3)}
                onChange={(e) =>
                  setEdit("focus", "maxSlots", Number.parseInt(e.target.value, 10) || 1)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">
                Deadline horizon (hours)
              </span>
              <p className="text-ui-xs text-fg-dim mb-1">
                Tasks due within this window are considered urgent
              </p>
              <input
                type="number"
                min={1}
                max={168}
                value={val("focus", "deadlineHours", 18)}
                onChange={(e) =>
                  setEdit("focus", "deadlineHours", Number.parseInt(e.target.value, 10) || 1)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            {isDirty("focus") && (
              <SaveButton onClick={() => save("focus")} saving={saving === "focus"} />
            )}
          </div>
        </SettingsCard>

        {/* ── Notifications ─────────────────────────────────────── */}
        <SettingsCard title="Notifications">
          <div className="space-y-3">
            <div>
              <span className="block text-ui-xs text-fg-secondary mb-1.5">
                Notification targets
              </span>
              <div className="flex flex-wrap gap-2">
                {NOTIFICATION_TARGET_OPTIONS.map((opt) => {
                  const active = currentTargets.includes(opt.value);
                  return (
                    <button
                      type="button"
                      key={opt.value}
                      onClick={() => toggleTarget(opt.value)}
                      className={`px-2.5 py-1 text-ui-xs rounded-md border transition-colors ${
                        active
                          ? "bg-brand/10 border-brand/30 text-brand"
                          : "bg-control-hover border-separator text-fg-secondary hover:border-fg-secondary/40"
                      }`}
                    >
                      {opt.label}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Focus reminders</span>
                <p className="text-ui-xs text-fg-dim">Remind you when a focused task is overdue</p>
              </div>
              <Toggle
                checked={val("notifications", "focusReminders", true)}
                onChange={(v) => setEdit("notifications", "focusReminders", v)}
              />
            </div>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Daily digest</span>
                <p className="text-ui-xs text-fg-dim">
                  Receive a summary of upcoming tasks each morning
                </p>
              </div>
              <Toggle
                checked={val("notifications", "dailyDigest", true)}
                onChange={(v) => setEdit("notifications", "dailyDigest", v)}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Daily digest time</span>
              <input
                type="time"
                value={val("notifications", "dailyDigestTime", "09:00")}
                onChange={(e) => setEdit("notifications", "dailyDigestTime", e.target.value)}
                className={`${INPUT_CLASS} w-32`}
              />
            </label>

            {isDirty("notifications") && (
              <SaveButton
                onClick={() => save("notifications")}
                saving={saving === "notifications"}
              />
            )}
          </div>
        </SettingsCard>

        {/* ── Enrichment ────────────────────────────────────────── */}
        <SettingsCard title="Task enrichment">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Auto-enrich tasks</span>
                <p className="text-ui-xs text-fg-dim">
                  Automatically infer priority, project, and tags on creation
                </p>
              </div>
              <Toggle
                checked={val("enrichment", "enabled", true)}
                onChange={(v) => setEdit("enrichment", "enabled", v)}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">
                Auto-apply confidence threshold
              </span>
              <p className="text-ui-xs text-fg-dim mb-1">
                Suggestions above this confidence are applied without confirmation (0.0–1.0)
              </p>
              <input
                type="number"
                min={0}
                max={1}
                step={0.05}
                value={val("enrichment", "autoApplyThreshold", 0.85)}
                onChange={(e) =>
                  setEdit(
                    "enrichment",
                    "autoApplyThreshold",
                    Number.parseFloat(e.target.value) || 0.85,
                  )
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Use LLM for enrichment</span>
                <p className="text-ui-xs text-fg-dim">
                  Use an AI model instead of keyword matching (uses tokens)
                </p>
              </div>
              <Toggle
                checked={val("enrichment", "useLlm", false)}
                onChange={(v) => setEdit("enrichment", "useLlm", v)}
              />
            </div>

            {isDirty("enrichment") && (
              <SaveButton onClick={() => save("enrichment")} saving={saving === "enrichment"} />
            )}
          </div>
        </SettingsCard>

        {/* ── Search ────────────────────────────────────────────── */}
        <SettingsCard title="Task search">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Semantic search</span>
                <p className="text-ui-xs text-fg-dim">
                  Enable meaning-based search in addition to keyword matching
                </p>
              </div>
              <Toggle
                checked={val("search", "enabled", true)}
                onChange={(v) => setEdit("search", "enabled", v)}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">
                Similarity threshold
              </span>
              <p className="text-ui-xs text-fg-dim mb-1">
                Minimum cosine similarity for semantic results (0.0–1.0)
              </p>
              <input
                type="number"
                min={0}
                max={1}
                step={0.05}
                value={val("search", "semanticThreshold", 0.5)}
                onChange={(e) =>
                  setEdit("search", "semanticThreshold", Number.parseFloat(e.target.value) || 0.5)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">Embedding model</span>
              <input
                type="text"
                value={val("search", "embeddingModel", "paraphrase-multilingual-MiniLM-L12-v2")}
                onChange={(e) => setEdit("search", "embeddingModel", e.target.value)}
                className={INPUT_CLASS}
              />
            </label>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-0.5">RRF k parameter</span>
              <p className="text-ui-xs text-fg-dim mb-1">
                Reciprocal rank fusion constant for hybrid search (higher = more weight on keyword)
              </p>
              <input
                type="number"
                min={1}
                max={1000}
                value={val("search", "rrfK", 60)}
                onChange={(e) =>
                  setEdit("search", "rrfK", Number.parseInt(e.target.value, 10) || 60)
                }
                className={`${INPUT_CLASS} w-24`}
              />
            </label>

            {isDirty("search") && (
              <SaveButton onClick={() => save("search")} saving={saving === "search"} />
            )}
          </div>
        </SettingsCard>

        {/* ── Daily Planning ────────────────────────────────────── */}
        <SettingsCard title="Daily planning">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-ui-xs text-fg-secondary">Enable daily planning</span>
                <p className="text-ui-xs text-fg-dim">
                  Automatically generate a daily plan each morning
                </p>
              </div>
              <Toggle
                checked={val("dailyPlanning", "enabled", true)}
                onChange={(v) => setEdit("dailyPlanning", "enabled", v)}
              />
            </div>

            <label className="block">
              <span className="block text-ui-xs text-fg-secondary mb-1">Planning time</span>
              <input
                type="time"
                value={val("dailyPlanning", "planningTime", "08:00")}
                onChange={(e) => setEdit("dailyPlanning", "planningTime", e.target.value)}
                className={`${INPUT_CLASS} w-32`}
              />
            </label>

            {isDirty("dailyPlanning") && (
              <SaveButton
                onClick={() => save("dailyPlanning")}
                saving={saving === "dailyPlanning"}
              />
            )}
          </div>
        </SettingsCard>
      </div>
    </div>
  );
}
