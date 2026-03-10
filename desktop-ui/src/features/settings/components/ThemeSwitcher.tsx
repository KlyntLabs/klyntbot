import { type Theme, useTheme } from "@app/providers/ThemeProvider";

const themes: { id: Theme; label: string; description: string }[] = [
  {
    id: "dark",
    label: "Tahoe Dark",
    description: "Modern glassmorphism with deep blacks and warm accents",
  },
  {
    id: "retro",
    label: "Retro 90s",
    description: "Warm cream tones, chunky borders, pixel-perfect nostalgia",
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
            className={`relative flex flex-col items-start gap-2 rounded-xl border-2 p-3 text-left transition-all ${
              theme === t.id ? "border-brand bg-brand/5" : "border-border hover:border-muted"
            }`}
          >
            {/* Preview swatch */}
            <div className="w-full h-20 rounded-lg overflow-hidden border border-border">
              {t.id === "dark" && <DarkPreview />}
              {t.id === "retro" && <RetroPreview />}
            </div>
            <div>
              <span className="text-sm font-medium text-primary">{t.label}</span>
              <p className="text-xs text-muted mt-0.5">{t.description}</p>
            </div>
            {theme === t.id && (
              <div className="absolute top-2 right-2 w-5 h-5 rounded-full bg-brand flex items-center justify-center">
                <svg
                  className="w-3 h-3 text-white"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={3}
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

/** Mini dark theme preview */
function DarkPreview() {
  return (
    <div className="w-full h-full bg-black p-2 flex gap-1.5">
      <div className="w-8 h-full rounded bg-white/[0.06] border border-white/[0.08]" />
      <div className="flex-1 flex flex-col gap-1">
        <div className="h-3 rounded bg-white/[0.06] border border-white/[0.08]" />
        <div className="flex-1 rounded bg-white/[0.04] border border-white/[0.06] p-1.5 flex flex-col gap-1">
          <div className="h-1.5 w-3/4 rounded-full bg-white/10" />
          <div className="h-1.5 w-1/2 rounded-full bg-white/10" />
          <div className="h-1.5 w-2/3 rounded-full bg-orange-500/30" />
        </div>
      </div>
    </div>
  );
}

/** Mini retro 90s preview */
function RetroPreview() {
  return (
    <div className="w-full h-full p-1.5 flex gap-1" style={{ background: "#f5f0e1" }}>
      <div
        className="w-7 h-full border border-black/80"
        style={{ background: "#ebe6d5", borderWidth: "1.5px" }}
      />
      <div className="flex-1 flex flex-col gap-1">
        {/* Title bar */}
        <div
          className="h-3 flex items-center px-1 gap-0.5"
          style={{ background: "#2663b0", borderRadius: "1px" }}
        >
          <div className="w-1.5 h-1.5 border border-white/60" style={{ borderWidth: "1px" }} />
          <div className="flex-1" />
          <div className="flex gap-px">
            <div
              className="w-2 h-2 border border-white/40"
              style={{ background: "#c4b89e", borderWidth: "1px" }}
            />
            <div
              className="w-2 h-2 border border-white/40"
              style={{ background: "#c4b89e", borderWidth: "1px" }}
            />
          </div>
        </div>
        {/* Content card */}
        <div
          className="flex-1 p-1 flex flex-col gap-0.5"
          style={{
            background: "#f0eadb",
            border: "1.5px solid #2a2520",
            boxShadow: "2px 2px 0px #2a2520",
          }}
        >
          <div className="h-1.5 w-3/4" style={{ background: "#2a2520" }} />
          <div className="h-1.5 w-1/2" style={{ background: "#7a7260" }} />
          <div className="h-1.5 w-2/3" style={{ background: "#e8740c" }} />
        </div>
      </div>
    </div>
  );
}
