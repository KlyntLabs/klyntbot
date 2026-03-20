import { SettingsCard } from "@shared/composites";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useToastContext } from "@shared/hooks/useToast";
import { SaveButton, Toggle } from "@shared/ui";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";

// ── Types ────────────────────────────────────────────────────────────

interface LauncherData {
  enabled?: boolean;
  sources?: {
    apps?: { enabled?: boolean };
    systemPrefs?: { enabled?: boolean };
    brew?: { enabled?: boolean };
    sshHosts?: { enabled?: boolean };
    gitRepos?: { enabled?: boolean; scanDirs?: string[] };
    scripts?: { enabled?: boolean; dir?: string };
    files?: { enabled?: boolean };
    contentGrep?: { enabled?: boolean; defaultScope?: string };
    contacts?: { enabled?: boolean };
    runningApps?: { enabled?: boolean };
    bookmarks?: { enabled?: boolean; browser?: string };
    browserHistory?: { enabled?: boolean; browser?: string; maxDays?: number };
    tasks?: { enabled?: boolean };
    notes?: { enabled?: boolean };
    clipboard?: { enabled?: boolean; maxEntries?: number };
  };
}

type SourceKey = keyof NonNullable<LauncherData["sources"]>;

interface SourceDef {
  key: SourceKey;
  label: string;
  extraFields?: { key: string; label: string; type: "text" | "number" | "dirs"; placeholder?: string }[];
}

const SOURCE_DEFS: SourceDef[] = [
  { key: "apps", label: "Applications" },
  { key: "systemPrefs", label: "System Preferences" },
  { key: "brew", label: "Homebrew packages" },
  { key: "sshHosts", label: "SSH hosts" },
  {
    key: "gitRepos",
    label: "Git repositories",
    extraFields: [{ key: "scanDirs", label: "Scan directories", type: "dirs", placeholder: "~/Projects" }],
  },
  {
    key: "scripts",
    label: "Scripts",
    extraFields: [{ key: "dir", label: "Scripts directory", type: "text", placeholder: "~/.klyntbot/scripts" }],
  },
  { key: "files", label: "Files" },
  {
    key: "contentGrep",
    label: "Content search (grep)",
    extraFields: [{ key: "defaultScope", label: "Default scope", type: "text", placeholder: "." }],
  },
  { key: "contacts", label: "Contacts" },
  { key: "runningApps", label: "Running apps" },
  {
    key: "bookmarks",
    label: "Browser bookmarks",
    extraFields: [{ key: "browser", label: "Browser", type: "text", placeholder: "chrome" }],
  },
  {
    key: "browserHistory",
    label: "Browser history",
    extraFields: [
      { key: "browser", label: "Browser", type: "text", placeholder: "chrome" },
      { key: "maxDays", label: "Max days", type: "number" },
    ],
  },
  { key: "tasks", label: "Tasks" },
  { key: "notes", label: "Notes" },
  {
    key: "clipboard",
    label: "Clipboard history",
    extraFields: [{ key: "maxEntries", label: "Max entries", type: "number" }],
  },
];

const INPUT_CLASS =
  "w-full px-3 py-1.5 text-[12px] text-foreground bg-accent border border-border rounded-md focus:outline-none focus:border-brand/50 transition-colors placeholder:text-dim";

// ── Component ────────────────────────────────────────────────────────

export function LauncherSettings() {
  const toast = useToastContext();
  const { data: launcher, refetch } = useQuery<LauncherData>(
    "config_get_section",
    { section: "launcher" },
    {},
  );

  const [edits, setEdits] = useState<Record<string, unknown>>({});
  const [sourceEdits, setSourceEdits] = useState<Partial<Record<SourceKey, Record<string, unknown>>>>({});
  const [saving, setSaving] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<SourceKey | null>(null);

  // ── Helpers ──────────────────────────────────────────────────────

  const sourceVal = <T,>(source: SourceKey, key: string, fallback: T): T => {
    const sEdits = sourceEdits[source];
    if (sEdits && key in sEdits) return sEdits[key] as T;
    const data = (launcher.sources as Record<string, Record<string, unknown>> | undefined)?.[source];
    return (data?.[key] as T) ?? fallback;
  };

  const setSourceEdit = (source: SourceKey, key: string, value: unknown) => {
    setSourceEdits((prev) => ({ ...prev, [source]: { ...prev[source], [key]: value } }));
  };

  const hasDirtyEdits = Object.keys(edits).length > 0;
  const hasDirtySources = Object.keys(sourceEdits).length > 0;

  const saveTop = async () => {
    if (!hasDirtyEdits) return;
    setSaving("top");
    try {
      await ipc("config_update_section", { section: "launcher", patch: edits });
      setEdits({});
      refetch();
    } catch {
      toast.show("Failed to save launcher settings");
    } finally {
      setSaving(null);
    }
  };

  const saveSources = async () => {
    if (!hasDirtySources) return;
    setSaving("sources");
    try {
      await ipc("config_update_section", {
        section: "launcher",
        patch: { sources: sourceEdits },
      });
      setSourceEdits({});
      refetch();
    } catch {
      toast.show("Failed to save source settings");
    } finally {
      setSaving(null);
    }
  };

  return (
    <div>
      <div className="mb-8">
        <h2 className="text-lg font-medium text-foreground">Launcher</h2>
        <p className="text-[13px] text-muted-foreground mt-1">
          Configure which sources the launcher searches
        </p>
      </div>

      <div className="space-y-4">
        <SettingsCard title="General">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-[12px] text-muted-foreground">Enable launcher</span>
                <p className="text-[11px] text-dim">Global toggle for the command launcher</p>
              </div>
              <Toggle
                checked={("enabled" in edits ? edits.enabled : launcher.enabled) as boolean ?? true}
                onChange={(v) => setEdits((prev) => ({ ...prev, enabled: v }))}
              />
            </div>
            {hasDirtyEdits && <SaveButton onClick={saveTop} saving={saving === "top"} />}
          </div>
        </SettingsCard>

        <SettingsCard title="Search sources">
          <div className="space-y-1.5">
            {SOURCE_DEFS.map((src) => {
              const isExpanded = expanded === src.key;
              const hasExtra = src.extraFields && src.extraFields.length > 0;
              const enabled = sourceVal(src.key, "enabled", true);

              return (
                <div key={src.key} className="bg-card rounded-lg border border-border-subtle">
                  <div className="flex items-center gap-2 p-3">
                    {hasExtra ? (
                      <button
                        type="button"
                        onClick={() => setExpanded(isExpanded ? null : src.key)}
                        className="text-muted-foreground hover:text-foreground transition-colors"
                      >
                        {isExpanded ? (
                          <ChevronDown className="w-3.5 h-3.5" />
                        ) : (
                          <ChevronRight className="w-3.5 h-3.5" />
                        )}
                      </button>
                    ) : (
                      <span className="w-3.5" />
                    )}
                    <span className="flex-1 text-[13px] font-medium text-muted-foreground">
                      {src.label}
                    </span>
                    <Toggle
                      size="sm"
                      checked={enabled as boolean}
                      onChange={(v) => setSourceEdit(src.key, "enabled", v)}
                    />
                  </div>

                  {isExpanded && hasExtra && (
                    <div className="px-3 pb-3 space-y-2 border-t border-border-subtle pt-2">
                      {src.extraFields!.map((field) => (
                        <label key={field.key} className="block">
                          <span className="block text-[11px] text-muted-foreground mb-1">
                            {field.label}
                          </span>
                          {field.type === "dirs" ? (
                            <input
                              type="text"
                              value={(sourceVal(src.key, field.key, []) as string[]).join(", ")}
                              onChange={(e) =>
                                setSourceEdit(
                                  src.key,
                                  field.key,
                                  e.target.value.split(",").map((s) => s.trim()).filter(Boolean),
                                )
                              }
                              placeholder={field.placeholder}
                              className={INPUT_CLASS}
                            />
                          ) : (
                            <input
                              type={field.type}
                              value={String(sourceVal(src.key, field.key, "") ?? "")}
                              onChange={(e) =>
                                setSourceEdit(
                                  src.key,
                                  field.key,
                                  field.type === "number"
                                    ? Number.parseInt(e.target.value, 10) || 0
                                    : e.target.value,
                                )
                              }
                              placeholder={field.placeholder}
                              className={`${INPUT_CLASS} ${field.type === "number" ? "w-24" : ""}`}
                            />
                          )}
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          {hasDirtySources && (
            <div className="mt-3">
              <SaveButton onClick={saveSources} saving={saving === "sources"} />
            </div>
          )}
        </SettingsCard>
      </div>
    </div>
  );
}
