import { type Theme, useTheme } from "@app/providers/ThemeProvider";

const themes: { id: Theme; label: string; description: string }[] = [
  {
    id: "light",
    label: "Light",
    description: "Clear glass surfaces on a bright canvas",
  },
  {
    id: "dark",
    label: "Dark",
    description: "Liquid glass on deep blacks with blue accent",
  },
];

export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme();

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-3">
        {themes.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTheme(t.id)}
            className={`relative flex flex-col items-start gap-2 rounded-panel border-2 p-3 text-left transition-all ${
              theme === t.id
                ? "border-brand bg-brand/5"
                : "border-separator hover:border-fg-secondary/40"
            }`}
          >
            <div className="w-full h-20 rounded-lg overflow-hidden border border-separator">
              {t.id === "light" ? <LightPreview /> : <DarkPreview />}
            </div>
            <div>
              <span className="text-ui font-medium text-fg">{t.label}</span>
              <p className="text-ui-xs text-fg-secondary mt-0.5">{t.description}</p>
            </div>
            {theme === t.id && (
              <div className="absolute top-2 right-2 size-5 rounded-full bg-brand flex items-center justify-center">
                <svg
                  className="size-3 text-brand-foreground"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={3}
                  aria-hidden="true"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                </svg>
              </div>
            )}
          </button>
        ))}
      </div>
    </div>
  );
}

function LightPreview() {
  return (
    <div className="w-full h-full bg-[#f2f2f4] p-2 flex gap-1.5">
      <div className="w-8 h-full rounded bg-white/70 border border-black/10" />
      <div className="flex-1 flex flex-col gap-1">
        <div className="h-3 rounded bg-white/80 border border-black/10" />
        <div className="flex-1 rounded bg-white/60 border border-black/8 p-1.5 flex flex-col gap-1">
          <div className="h-1.5 w-3/4 rounded-full bg-black/15" />
          <div className="h-1.5 w-1/2 rounded-full bg-black/10" />
          <div className="h-1.5 w-2/3 rounded-full bg-[#0a7cff]/40" />
        </div>
      </div>
    </div>
  );
}

function DarkPreview() {
  return (
    <div className="w-full h-full bg-black p-2 flex gap-1.5">
      <div className="w-8 h-full rounded bg-white/[0.06] border border-white/[0.08]" />
      <div className="flex-1 flex flex-col gap-1">
        <div className="h-3 rounded bg-white/[0.06] border border-white/[0.08]" />
        <div className="flex-1 rounded bg-white/[0.04] border border-white/[0.06] p-1.5 flex flex-col gap-1">
          <div className="h-1.5 w-3/4 rounded-full bg-white/10" />
          <div className="h-1.5 w-1/2 rounded-full bg-white/10" />
          <div className="h-1.5 w-2/3 rounded-full bg-[#0a7cff]/40" />
        </div>
      </div>
    </div>
  );
}
