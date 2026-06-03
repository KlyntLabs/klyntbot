import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";

const GITHUB_URL = "https://github.com/KlyntLabs/klyntbot";
const TWITTER_URL = "https://x.com/jayden_dangvu";
const ARCHITECTURE_URL =
  "https://github.com/KlyntLabs/klyntbot/blob/main/docs/architecture/00-overview.md";

export function AboutView() {
  const [version, setVersion] = useState<string | null>(null);

  const handleOpenGitHub = () => {
    void openUrl(GITHUB_URL);
  };

  const handleOpenArchitecture = () => {
    void openUrl(ARCHITECTURE_URL);
  };

  const handleOpenTwitter = () => {
    void openUrl(TWITTER_URL);
  };

  useEffect(() => {
    let active = true;
    const fetchVersion = async () => {
      try {
        const value = await getVersion();
        if (active) {
          setVersion(value);
        }
      } catch {
        if (active) {
          setVersion(null);
        }
      }
    };

    void fetchVersion();
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-surface-messages">
      <div className="flex flex-col items-center gap-4 p-10 rounded-2xl bg-surface-card border border-border-subtle shadow-[0_24px_48px_rgba(0,0,0,0.25)]">
        <div className="flex items-center gap-2.5">
          <img className="w-11 h-11 rounded-lg" src="/app-icon.png" alt="KlyntBot icon" />
          <div className="text-[22px] font-bold tracking-wide text-text-strong">KlyntBot</div>
        </div>
        <div className="text-ui-sm text-text-faint">{version ? `Version ${version}` : "Version —"}</div>
        <div className="text-ui-sm text-text-muted max-w-[260px] text-center">Personal cognitive agent OS</div>
        <div className="w-40 h-px bg-border-subtle my-1" />
        <div className="flex items-center gap-2 text-ui-sm text-text-muted">
          <button type="button" className="bg-transparent border-none text-text-muted cursor-pointer hover:text-text-strong transition-colors" onClick={handleOpenGitHub}>
            GitHub
          </button>
          <span className="text-text-faint">|</span>
          <button type="button" className="bg-transparent border-none text-text-muted cursor-pointer hover:text-text-strong transition-colors" onClick={handleOpenArchitecture}>
            Architecture
          </button>
          <span className="text-text-faint">|</span>
          <button type="button" className="bg-transparent border-none text-text-muted cursor-pointer hover:text-text-strong transition-colors" onClick={handleOpenTwitter}>
            Twitter
          </button>
        </div>
        <div className="text-ui-2xs text-text-faint mt-2">Made with ♥ by KlyntBot</div>
      </div>
    </div>
  );
}
