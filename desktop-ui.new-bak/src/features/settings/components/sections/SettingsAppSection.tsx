import {
  NumberField,
  SelectField,
  SliderField,
  TextField,
  ToggleField,
} from "@settings/components/fields";
import type { ThemePreference, UiConfig } from "@settings/lib/configSectionTypes";
import { useConfigSection } from "@settings/lib/useConfigSection";
import { useCallback, useEffect, useState } from "react";

const THEME_OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "dim", label: "Dim" },
];

const DEFAULTS: Required<UiConfig> = {
  theme: "system",
  uiScale: 1.0,
  uiFontFamily: "Inter",
  codeFontFamily: "JetBrains Mono",
  codeFontSize: 11,
  notificationSoundsEnabled: true,
  systemNotificationsEnabled: true,
  subagentSystemNotificationsEnabled: true,
  threadTitleAutogenerationEnabled: false,
  automaticAppUpdateChecksEnabled: true,
  chatHistoryScrollbackItems: 100,
  showMessageFilePath: true,
  splitChatDiffView: false,
};

export function SettingsAppSection() {
  const { value, loading, error, patching, patch } = useConfigSection<UiConfig>("ui");

  const [draft, setDraft] = useState<UiConfig>(DEFAULTS);
  const [initialized, setInitialized] = useState(false);

  useEffect(() => {
    if (value && !initialized) {
      setDraft({ ...DEFAULTS, ...value });
      setInitialized(true);
    }
  }, [value, initialized]);

  const updateDraft = useCallback(<K extends keyof UiConfig>(key: K, val: UiConfig[K]) => {
    setDraft((d) => ({ ...d, [key]: val }));
  }, []);

  const commit = useCallback(
    async (patchValue: Partial<UiConfig>) => {
      await patch(patchValue);
    },
    [patch],
  );

  if (loading) {
    return <div className="text-[var(--fs-sm)] text-[var(--text-subtle)]">Loading…</div>;
  }

  if (error && !patching) {
    return (
      <div className="rounded-lg border border-red-400/30 bg-red-400/10 p-4 text-[var(--fs-sm)] text-red-400">
        {error}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <h2 className="text-[var(--fs-lg)] font-semibold text-[var(--text-strong)]">App &amp; UI</h2>

      {patching && <div className="text-[var(--fs-xs)] text-[var(--text-subtle)]">Saving…</div>}

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">
          Appearance
        </h3>

        <SelectField
          label="Theme"
          value={draft.theme ?? "system"}
          options={THEME_OPTIONS}
          onChange={(v) => {
            updateDraft("theme", v as ThemePreference);
            commit({ theme: v as ThemePreference });
          }}
        />

        <SliderField
          label="UI scale"
          value={draft.uiScale ?? 1.0}
          min={0.75}
          max={1.5}
          step={0.05}
          onChange={(v) => updateDraft("uiScale", v)}
          onBlur={() => commit({ uiScale: draft.uiScale })}
        />

        <TextField
          label="UI font family"
          value={draft.uiFontFamily ?? "Inter"}
          onChange={(v) => updateDraft("uiFontFamily", v)}
          onBlur={() => commit({ uiFontFamily: draft.uiFontFamily })}
        />

        <TextField
          label="Code font family"
          value={draft.codeFontFamily ?? "JetBrains Mono"}
          onChange={(v) => updateDraft("codeFontFamily", v)}
          onBlur={() => commit({ codeFontFamily: draft.codeFontFamily })}
        />

        <NumberField
          label="Code font size"
          value={draft.codeFontSize ?? 11}
          min={8}
          max={32}
          step={1}
          onChange={(v) => updateDraft("codeFontSize", v)}
          onBlur={() => commit({ codeFontSize: draft.codeFontSize })}
        />
      </div>

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">
          Notifications
        </h3>

        <ToggleField
          label="Notification sounds"
          value={draft.notificationSoundsEnabled ?? true}
          onChange={(v) => {
            updateDraft("notificationSoundsEnabled", v);
            commit({ notificationSoundsEnabled: v });
          }}
        />

        <ToggleField
          label="System notifications"
          description="Show OS-native notification banners"
          value={draft.systemNotificationsEnabled ?? true}
          onChange={(v) => {
            updateDraft("systemNotificationsEnabled", v);
            commit({ systemNotificationsEnabled: v });
          }}
        />

        <ToggleField
          label="Subagent system notifications"
          description="Show OS-native banners for subagent activity"
          value={draft.subagentSystemNotificationsEnabled ?? true}
          onChange={(v) => {
            updateDraft("subagentSystemNotificationsEnabled", v);
            commit({ subagentSystemNotificationsEnabled: v });
          }}
        />
      </div>

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">Display</h3>

        <NumberField
          label="Chat scrollback items"
          description="Number of messages to keep in memory"
          value={draft.chatHistoryScrollbackItems ?? 100}
          min={1}
          max={10000}
          step={1}
          onChange={(v) => updateDraft("chatHistoryScrollbackItems", v)}
          onBlur={() => commit({ chatHistoryScrollbackItems: draft.chatHistoryScrollbackItems })}
        />

        <ToggleField
          label="Show message file path"
          value={draft.showMessageFilePath ?? true}
          onChange={(v) => {
            updateDraft("showMessageFilePath", v);
            commit({ showMessageFilePath: v });
          }}
        />

        <ToggleField
          label="Split chat diff view"
          description="Show diffs side-by-side instead of inline"
          value={draft.splitChatDiffView ?? false}
          onChange={(v) => {
            updateDraft("splitChatDiffView", v);
            commit({ splitChatDiffView: v });
          }}
        />
      </div>

      <div className="rounded-lg border border-[var(--border-subtle)] p-4">
        <h3 className="mb-2 text-[var(--fs-md)] font-medium text-[var(--text-strong)]">
          Behaviour
        </h3>

        <ToggleField
          label="Auto-generate thread titles"
          description="Generate titles from the first message"
          value={draft.threadTitleAutogenerationEnabled ?? false}
          onChange={(v) => {
            updateDraft("threadTitleAutogenerationEnabled", v);
            commit({ threadTitleAutogenerationEnabled: v });
          }}
        />

        <ToggleField
          label="Automatic update checks"
          description="Check for app updates on startup"
          value={draft.automaticAppUpdateChecksEnabled ?? true}
          onChange={(v) => {
            updateDraft("automaticAppUpdateChecksEnabled", v);
            commit({ automaticAppUpdateChecksEnabled: v });
          }}
        />
      </div>
    </div>
  );
}
