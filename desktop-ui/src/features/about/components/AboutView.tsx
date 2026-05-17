import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import "@/styles/about.css";

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
    <div className="about">
      <div className="about-card">
        <div className="about-header">
          <img className="about-icon" src="/app-icon.png" alt="KlyntBot icon" />
          <div className="about-title">KlyntBot</div>
        </div>
        <div className="about-version">{version ? `Version ${version}` : "Version —"}</div>
        <div className="about-tagline">Personal cognitive agent OS</div>
        <div className="about-divider" />
        <div className="about-links">
          <button type="button" className="about-link" onClick={handleOpenGitHub}>
            GitHub
          </button>
          <span className="about-link-sep">|</span>
          <button type="button" className="about-link" onClick={handleOpenArchitecture}>
            Architecture
          </button>
          <span className="about-link-sep">|</span>
          <button type="button" className="about-link" onClick={handleOpenTwitter}>
            Twitter
          </button>
        </div>
        <div className="about-footer">Made with ♥ by KlyntBot</div>
      </div>
    </div>
  );
}
