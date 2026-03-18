/**
 * Shared constants and components for productivity widgets.
 */

import { AppWindow } from "lucide-react";
import type { JSX } from "react";

// ── App brand colors & icons ──────────────────────────────────────────

/** Brand colors for popular apps (lowercase keys).
 *  Includes both native app names and domain-based site names
 *  (e.g. "youtube.com") since the tracker returns domains for browser sites. */
export const APP_COLORS: Record<string, string> = {
  // Native apps (matched by app_name)
  chrome: "#4285F4",
  "google chrome": "#4285F4",
  firefox: "#FF7139",
  safari: "#006CFF",
  arc: "#5B5FC7",
  "visual studio code": "#007ACC",
  code: "#007ACC",
  cursor: "#7C3AED",
  xcode: "#147EFB",
  intellij: "#FE315D",
  "intellij idea": "#FE315D",
  webstorm: "#07C3F2",
  terminal: "#4D4D4D",
  iterm2: "#2bbb4f",
  warp: "#01A4FF",
  ghostty: "#7dd3fc",
  alacritty: "#F4A82A",
  kitty: "#7E57C2",
  figma: "#F24E1E",
  sketch: "#FDAD00",
  linear: "#5E6AD2",
  notion: "#FFFFFFCC",
  obsidian: "#7C3AED",
  slack: "#4A154B",
  discord: "#5865F2",
  telegram: "#26A5E4",
  whatsapp: "#25D366",
  zoom: "#2D8CFF",
  spotify: "#1DB954",
  docker: "#2496ED",
  postman: "#FF6C37",
  tableplus: "#F8A917",
  tower: "#5D6872",
  "sublime text": "#FF9800",
  finder: "#4AABEE",
  messages: "#34C759",
  mail: "#007AFF",
  notes: "#FFCC00",
  calendar: "#FF3B30",
  preview: "#2B7DF7",
  "system preferences": "#8E8E93",
  "system settings": "#8E8E93",
  "github desktop": "#E6EDF3",
  desktop: "#8E8E93",

  // Domain-based site names (matched by site_name from tracker)
  "youtube.com": "#FF0000",
  "github.com": "#E6EDF3",
  "reddit.com": "#FF4500",
  "twitter.com": "#1DA1F2",
  "x.com": "#E7E9EA",
  "spotify.com": "#1DB954",
  "netflix.com": "#E50914",
  "twitch.tv": "#9146FF",
  "facebook.com": "#1877F2",
  "instagram.com": "#E4405F",
  "tiktok.com": "#000000",
  "linkedin.com": "#0A66C2",
  "pinterest.com": "#E60023",
  "claude.ai": "#D4A27F",
  "anthropic.com": "#D4A27F",
  "chatgpt.com": "#10A37F",
  "openai.com": "#10A37F",
  "perplexity.ai": "#1FB8CD",
  "gemini.google.com": "#4285F4",
  "slack.com": "#4A154B",
  "discord.com": "#5865F2",
  "telegram.org": "#26A5E4",
  "whatsapp.com": "#25D366",
  "zoom.us": "#2D8CFF",
  "meet.google.com": "#00897B",
  "teams.microsoft.com": "#6264A7",
  "figma.com": "#F24E1E",
  "notion.so": "#FFFFFFCC",
  "linear.app": "#5E6AD2",
  "docs.google.com": "#4285F4",
  "sheets.google.com": "#0F9D58",
  "slides.google.com": "#F4B400",
  "drive.google.com": "#4285F4",
  "gmail.com": "#EA4335",
  "calendar.google.com": "#4285F4",
  "maps.google.com": "#34A853",
  "google.com": "#4285F4",
  "amazon.com": "#FF9900",
  "ebay.com": "#E53238",
  "news.ycombinator.com": "#FF6600",
  "wikipedia.org": "#636466",
  "medium.com": "#FFFFFFCC",
  "stackoverflow.com": "#F48024",
  "gitlab.com": "#FC6D26",
  "vercel.com": "#FFFFFFCC",
  "netlify.com": "#00C7B7",
  "supabase.com": "#3ECF8E",
  "aws.amazon.com": "#FF9900",
  "azure.com": "#0078D4",
  "docker.com": "#2496ED",
  "sentry.io": "#362D59",
  "crates.io": "#FFC933",
  "docs.rs": "#27AE60",
  "npmjs.com": "#CB3837",
  "pypi.org": "#3775A9",
  localhost: "#8E8E93",
  "bsky.app": "#0085FF",
  "mastodon.social": "#6364FF",
};

// Shared SVG icon elements — defined once, referenced by multiple keys.
const chromeIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <circle cx="8" cy="8" r="7" fill="none" stroke="#4285F4" strokeWidth="1.5" />
    <circle cx="8" cy="8" r="3" fill="#4285F4" />
    <path d="M8 5L13.2 5" stroke="#DB4437" strokeWidth="1.5" strokeLinecap="round" />
    <path d="M5.4 9.5L2.8 4.8" stroke="#F4B400" strokeWidth="1.5" strokeLinecap="round" />
    <path d="M10.6 9.5L8 14" stroke="#0F9D58" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);
const vscodeIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <path d="M11.5 1L4.5 7.5L2 5.5L1 6.5L4.5 9.5L11.5 3V1Z" fill="#007ACC" />
    <path d="M11.5 3L4.5 9.5L2 7.5L1 8.5L4.5 11.5L11.5 5V3Z" fill="#007ACC" opacity="0.7" />
    <rect x="11" y="1" width="2" height="14" rx="0.5" fill="#007ACC" />
  </svg>
);
const githubIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <path
      fillRule="evenodd"
      d="M8 1C4.13 1 1 4.13 1 8c0 3.09 2 5.71 4.78 6.64.35.06.48-.15.48-.34 0-.17-.01-.71-.01-1.29-1.76.33-2.2-.43-2.34-.82-.08-.2-.42-.82-.71-.98-.24-.13-.59-.46-.01-.47.55-.01.94.51 1.07.71.63 1.05 1.63.76 2.03.58.06-.46.24-.76.44-.93-1.54-.18-3.15-.77-3.15-3.43 0-.76.27-1.38.71-1.87-.07-.17-.31-.88.07-1.84 0 0 .58-.18 1.9.71.55-.15 1.14-.23 1.73-.23.59 0 1.18.08 1.73.23 1.32-.9 1.9-.71 1.9-.71.38.96.14 1.67.07 1.84.44.49.71 1.11.71 1.87 0 2.67-1.62 3.25-3.16 3.42.25.21.46.63.46 1.27 0 .92-.01 1.66-.01 1.89 0 .19.13.41.48.34A7.003 7.003 0 0015 8c0-3.87-3.13-7-7-7z"
      fill="#E6EDF3"
    />
  </svg>
);
const slackIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <path
      d="M3.5 9.5C2.7 9.5 2 10.2 2 11C2 11.8 2.7 12.5 3.5 12.5H4.5V11C4.5 10.2 3.8 9.5 3 9.5"
      fill="#E01E5A"
    />
    <path
      d="M6 9.5H8.5C9.3 9.5 10 10.2 10 11V13.5C10 14.3 9.3 15 8.5 15C7.7 15 7 14.3 7 13.5V12.5H6C5.2 12.5 4.5 11.8 4.5 11C4.5 10.2 5.2 9.5 6 9.5"
      fill="#E01E5A"
    />
    <path
      d="M6.5 3.5C6.5 2.7 5.8 2 5 2C4.2 2 3.5 2.7 3.5 3.5V6C3.5 6.8 4.2 7.5 5 7.5C5.8 7.5 6.5 6.8 6.5 6V3.5Z"
      fill="#36C5F0"
    />
    <path
      d="M12.5 6.5C13.3 6.5 14 5.8 14 5C14 4.2 13.3 3.5 12.5 3.5H10C9.2 3.5 8.5 4.2 8.5 5C8.5 5.8 9.2 6.5 10 6.5H12.5Z"
      fill="#2EB67D"
    />
    <path
      d="M9.5 12.5C9.5 13.3 10.2 14 11 14C11.8 14 12.5 13.3 12.5 12.5V10C12.5 9.2 11.8 8.5 11 8.5C10.2 8.5 9.5 9.2 9.5 10V12.5Z"
      fill="#ECB22E"
    />
  </svg>
);
const youtubeIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="1" y="3" width="14" height="10" rx="3" fill="#FF0000" />
    <path d="M6.5 6L10.5 8L6.5 10V6Z" fill="white" />
  </svg>
);
const redditIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <circle cx="8" cy="9" r="6" fill="#FF4500" />
    <circle cx="6" cy="8.5" r="1" fill="white" />
    <circle cx="10" cy="8.5" r="1" fill="white" />
    <path
      d="M6 11C6.8 11.8 9.2 11.8 10 11"
      stroke="white"
      strokeWidth="0.8"
      fill="none"
      strokeLinecap="round"
    />
    <circle cx="11.5" cy="3" r="1.2" fill="#E6EDF3" />
    <path d="M10 5L11.5 3" stroke="#E6EDF3" strokeWidth="0.8" />
  </svg>
);
const discordIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="1" y="3" width="14" height="10" rx="3" fill="#5865F2" />
    <path
      d="M6 7.5C6.55 7.5 7 8 7 8.5C7 9 6.55 9.5 6 9.5C5.45 9.5 5 9 5 8.5C5 8 5.45 7.5 6 7.5Z"
      fill="white"
    />
    <path
      d="M10 7.5C10.55 7.5 11 8 11 8.5C11 9 10.55 9.5 10 9.5C9.45 9.5 9 9 9 8.5C9 8 9.45 7.5 10 7.5Z"
      fill="white"
    />
  </svg>
);
const dockerIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="1" y="6" width="14" height="7" rx="2" fill="#2496ED" />
    <rect x="3" y="3" width="2.5" height="2.5" rx="0.4" fill="#2496ED" />
    <rect x="6.5" y="3" width="2.5" height="2.5" rx="0.4" fill="#2496ED" />
    <rect x="3" y="7.5" width="2" height="2" rx="0.3" fill="white" opacity="0.8" />
    <rect x="6" y="7.5" width="2" height="2" rx="0.3" fill="white" opacity="0.8" />
    <rect x="9" y="7.5" width="2" height="2" rx="0.3" fill="white" opacity="0.8" />
  </svg>
);
const spotifyIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <circle cx="8" cy="8" r="7" fill="#1DB954" />
    <path
      d="M4.5 9.5C6.5 8.8 9.5 9 11.5 10"
      stroke="white"
      strokeWidth="1.2"
      strokeLinecap="round"
      fill="none"
    />
    <path
      d="M4 7.5C6.5 6.5 10 6.8 12 8"
      stroke="white"
      strokeWidth="1.2"
      strokeLinecap="round"
      fill="none"
    />
    <path
      d="M3.5 5.5C6.5 4.3 10.5 4.8 12.5 6"
      stroke="white"
      strokeWidth="1.2"
      strokeLinecap="round"
      fill="none"
    />
  </svg>
);
const notionIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="3" y="2" width="10" height="12" rx="2" fill="white" opacity="0.9" />
    <path d="M5.5 5H10.5" stroke="#333" strokeWidth="1" strokeLinecap="round" />
    <path d="M5.5 7.5H9" stroke="#333" strokeWidth="1" strokeLinecap="round" />
    <path d="M5.5 10H8" stroke="#333" strokeWidth="1" strokeLinecap="round" />
  </svg>
);
const netflixIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="4" y="2" width="3" height="12" rx="0.5" fill="#E50914" />
    <rect x="9" y="2" width="3" height="12" rx="0.5" fill="#E50914" />
    <path d="M4 2L12 14" stroke="#E50914" strokeWidth="3" />
  </svg>
);
const anthropicIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <path d="M8 2L13 13H10.5L8 7L5.5 13H3L8 2Z" fill="#D4A27F" />
  </svg>
);
const linearIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="2" y="2" width="12" height="12" rx="3" fill="#5E6AD2" />
    <path d="M5 11L11 5" stroke="white" strokeWidth="1.5" strokeLinecap="round" />
    <path d="M5 8L8 5" stroke="white" strokeWidth="1.5" strokeLinecap="round" opacity="0.6" />
  </svg>
);
const figmaIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <circle cx="9.5" cy="8" r="2.5" fill="#1ABCFE" />
    <path
      d="M4.5 13C4.5 11.6 5.6 10.5 7 10.5H9.5V13C9.5 14.4 8.4 15.5 7 15.5C5.6 15.5 4.5 14.4 4.5 13Z"
      fill="#0ACF83"
    />
    <path d="M4.5 8C4.5 6.6 5.6 5.5 7 5.5H9.5V10.5H7C5.6 10.5 4.5 9.4 4.5 8Z" fill="#A259FF" />
    <path d="M4.5 3C4.5 1.6 5.6 0.5 7 0.5H9.5V5.5H7C5.6 5.5 4.5 4.4 4.5 3Z" fill="#F24E1E" />
    <path
      d="M9.5 0.5H12C13.4 0.5 14.5 1.6 14.5 3C14.5 4.4 13.4 5.5 12 5.5H9.5V0.5Z"
      fill="#FF7262"
    />
  </svg>
);
const facebookIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <circle cx="8" cy="8" r="7" fill="#1877F2" />
    <path
      d="M10.5 8H9V13H7V8H5.5V6.5H7V5.5C7 4.1 7.8 3 9.5 3H10.5V4.5H9.8C9.2 4.5 9 4.8 9 5.3V6.5H10.5L10.5 8Z"
      fill="white"
    />
  </svg>
);
const instagramIcon = (
  <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
    <rect x="2" y="2" width="12" height="12" rx="4" fill="url(#ig)" />
    <circle cx="8" cy="8" r="3" fill="none" stroke="white" strokeWidth="1.2" />
    <circle cx="11.5" cy="4.5" r="0.8" fill="white" />
    <defs>
      <linearGradient id="ig" x1="2" y1="14" x2="14" y2="2">
        <stop stopColor="#FFDC80" />
        <stop offset="0.5" stopColor="#F56040" />
        <stop offset="1" stopColor="#833AB4" />
      </linearGradient>
    </defs>
  </svg>
);

/** SVG icon paths for popular apps (16×16 viewBox).
 *  Keys include both native app names and domain names. */
export const APP_ICONS: Record<string, JSX.Element> = {
  // Native app names
  chrome: chromeIcon,
  "google chrome": chromeIcon,
  "visual studio code": vscodeIcon,
  code: vscodeIcon,
  cursor: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <rect x="2" y="2" width="12" height="12" rx="3" fill="#7C3AED" />
      <path d="M6 6L10 8L6 10Z" fill="white" />
    </svg>
  ),
  ghostty: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <path
        d="M4 3C4 2 5 1 8 1C11 1 12 2 12 3V10C12 12 11 14 10 15H9V13C9 12.5 8.5 12 8 12C7.5 12 7 12.5 7 13V15H6C5 14 4 12 4 10V3Z"
        fill="#7dd3fc"
      />
      <circle cx="6.5" cy="5" r="1" fill="#0c4a6e" />
      <circle cx="9.5" cy="5" r="1" fill="#0c4a6e" />
    </svg>
  ),
  slack: slackIcon,
  youtube: youtubeIcon,
  github: githubIcon,
  "github desktop": githubIcon,
  linear: linearIcon,
  figma: figmaIcon,
  reddit: redditIcon,
  discord: discordIcon,
  docker: dockerIcon,
  spotify: spotifyIcon,
  notion: notionIcon,
  netflix: netflixIcon,
  anthropic: anthropicIcon,
  facebook: facebookIcon,
  instagram: instagramIcon,
  safari: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <circle cx="8" cy="8" r="7" fill="none" stroke="#006CFF" strokeWidth="1.5" />
      <path d="M8 3L10 7L8 8L6 12L8 8L10 7Z" fill="#006CFF" />
      <path d="M8 8L6 12L8 8L10 7Z" fill="#FF3B30" />
    </svg>
  ),
  arc: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <path
        d="M3 12C3 7.5 5 3 8 3C11 3 13 7.5 13 12"
        stroke="#5B5FC7"
        strokeWidth="2.5"
        strokeLinecap="round"
        fill="none"
      />
    </svg>
  ),
  finder: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <rect x="2" y="2" width="12" height="12" rx="3" fill="#4AABEE" />
      <path d="M5 6V10" stroke="white" strokeWidth="1.2" strokeLinecap="round" />
      <path d="M11 6V10" stroke="white" strokeWidth="1.2" strokeLinecap="round" />
      <path d="M5 8H11" stroke="white" strokeWidth="0.8" />
    </svg>
  ),
  desktop: (
    <svg viewBox="0 0 16 16" className="w-3.5 h-3.5" aria-hidden="true">
      <rect
        x="2"
        y="3"
        width="12"
        height="8"
        rx="1.5"
        fill="none"
        stroke="#8E8E93"
        strokeWidth="1.2"
      />
      <path d="M6 13H10" stroke="#8E8E93" strokeWidth="1.2" strokeLinecap="round" />
      <path d="M8 11V13" stroke="#8E8E93" strokeWidth="1.2" />
    </svg>
  ),
  // Domain-based site names (reuse shared icon variables)
  "youtube.com": youtubeIcon,
  "github.com": githubIcon,
  "reddit.com": redditIcon,
  "discord.com": discordIcon,
  "slack.com": slackIcon,
  "spotify.com": spotifyIcon,
  "netflix.com": netflixIcon,
  "facebook.com": facebookIcon,
  "instagram.com": instagramIcon,
  "claude.ai": anthropicIcon,
  "anthropic.com": anthropicIcon,
  "figma.com": figmaIcon,
  "notion.so": notionIcon,
  "linear.app": linearIcon,
  "docker.com": dockerIcon,
};

/** Resolve app brand color, falling back to category color or brand. */
export function getAppColor(appName: string, category: string | null): string {
  const key = appName.toLowerCase();
  if (APP_COLORS[key]) return APP_COLORS[key];
  if (category) return getCategoryColor(category);
  return "var(--brand)";
}

/** Render an app's brand icon, falling back to a generic AppWindow icon. */
export function AppIcon({ appName, color }: { appName: string; color: string }) {
  const key = appName.toLowerCase();
  const icon = APP_ICONS[key];
  if (icon) return icon;
  return <AppWindow className="w-3.5 h-3.5 flex-shrink-0" strokeWidth={1.5} style={{ color }} />;
}

// ── Category colors ───────────────────────────────────────────────────

/** Unique color per category — visually distinct on dark backgrounds. */
const CATEGORY_COLORS: Record<string, string> = {
  coding: "#22C55E",
  design: "#A78BFA",
  communication: "#F59E0B",
  entertainment: "#F87171",
  project_management: "#8B5CF6",
  documentation: "#60A5FA",
  email: "#78716C",
  browsing: "#94A3B8",
  ai_tools: "#06B6D4",
  social_media: "#F43F5E",
  video_streaming: "#EF4444",
  news_forums: "#FB923C",
  developer_tools: "#10B981",
  cloud_devops: "#34D399",
  shopping: "#FB7185",
  finance: "#A1A1AA",
  learning: "#2DD4BF",
  music: "#C084FC",
  gaming: "#E11D48",
};

const FALLBACK_COLORS = ["#60A5FA", "#A78BFA", "#F59E0B", "#22C55E", "#94A3B8", "#F43F5E"];

/** Default color for new or uncolored categories. */
export const DEFAULT_CATEGORY_COLOR = "#94A3B8";

/** Category type groups for grouping by productive/neutral/distracting. */
export const CATEGORY_TYPE_GROUPS: { type: string; label: string }[] = [
  { type: "productive", label: "Work" },
  { type: "neutral", label: "Utilities" },
  { type: "distracting", label: "Distraction" },
];

/** Type badge colors: productive (green), neutral (slate), distracting (rose). */
export const TYPE_BADGE_COLORS: Record<string, string> = {
  productive: "#22C55E",
  neutral: "#94A3B8",
  distracting: "#F43F5E",
};

/**
 * Resolve a category color from either an ID ("coding") or display name ("Coding").
 * Falls back to a rotating palette, then to slate.
 */
export function getCategoryColor(nameOrId: string, index = 0): string {
  const key = nameOrId.toLowerCase().replace(/[ &]/g, "_");
  return CATEGORY_COLORS[key] ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length];
}

/** Get the type badge color for a category type. */
export function getCategoryTypeColor(categoryType: string): string {
  return TYPE_BADGE_COLORS[categoryType] ?? TYPE_BADGE_COLORS.neutral;
}

/** Resolve activity block color from category type. Used by timeline and activity track. */
export function resolveActivityColor(categoryType: string | undefined, isIdle: boolean): string {
  if (isIdle) return "var(--surface-highest)";
  if (categoryType === "productive") return "var(--success)";
  if (categoryType === "distracting") return "var(--destructive)";
  if (categoryType === "neutral") return "var(--text-muted-foreground)";
  return "var(--brand)";
}

/**
 * Map a quality score (0–100) to a perceptually smooth color.
 * 0 → red, 50 → amber/yellow, 100 → green.
 * Uses oklch for perceptual uniformity.
 */
export function qualityToColor(score: number): string {
  const clamped = Math.max(0, Math.min(100, score));
  // Hue: 25° (red-orange) → 85° (yellow-green) → 145° (green)
  const hue = 25 + (clamped / 100) * 120;
  // Lightness: slightly brighter for higher scores
  const lightness = 0.52 + (clamped / 100) * 0.1;
  const chroma = 0.16 + (clamped / 100) * 0.04;
  return `oklch(${lightness.toFixed(2)} ${chroma.toFixed(2)} ${hue.toFixed(0)})`;
}

/**
 * Compute opacity from category purity (0.0–1.0).
 * Higher purity = more opaque (clearer signal of sustained focus).
 */
export function purityToOpacity(purity: number | null | undefined): number {
  if (purity == null) return 0.65;
  return 0.5 + Math.min(1, purity) * 0.4; // range: 0.5 – 0.9
}

/** Resolve category type to a human-readable label. */
export function resolveCategoryLabel(categoryType: string): string {
  if (categoryType === "productive") return "Productive";
  if (categoryType === "distracting") return "Distracting";
  if (categoryType === "neutral") return "Neutral";
  return "Uncategorized";
}

// scoreColor lives in constants.tsx — import from there.
export { scoreColor } from "./constants";

/** Build the standard Focus/Active/Breaks donut segments. */
export function buildBreakdownSegments(
  totalActive: number,
  totalFocus: number,
  totalBreak: number,
): { name: string; value: number; color: string }[] {
  return [
    { name: "Focus", value: totalFocus, color: "var(--brand)" },
    { name: "Active", value: totalActive - totalFocus - totalBreak, color: "var(--purple)" },
    { name: "Breaks", value: totalBreak, color: "var(--info)" },
  ];
}

/** Productivity legend items for bar charts. */
export const PRODUCTIVITY_LEGEND = [
  { label: "Productive", color: "var(--success)" },
  { label: "Neutral", color: "var(--text-muted-foreground)" },
  { label: "Distracting", color: "var(--destructive)" },
] as const;

interface TooltipPayloadEntry {
  dataKey: string;
  value: number;
  fill: string;
}

interface ChartTooltipProps {
  active?: boolean;
  payload?: TooltipPayloadEntry[];
  label?: string;
}

/** Shared tooltip for recharts bar charts. */
export function ChartTooltip({ active, payload, label }: ChartTooltipProps) {
  if (!active || !payload?.length) return null;
  const total = payload.reduce((s: number, p: TooltipPayloadEntry) => s + (p.value || 0), 0);
  return (
    <div
      className="rounded-lg px-3 py-2 text-[11px]"
      style={{
        background: "var(--surface-floating)",
        border: "1px solid var(--border)",
        boxShadow: "var(--shadow-tooltip)",
      }}
    >
      <div className="font-medium text-foreground mb-1">{label}</div>
      {payload.map((p: TooltipPayloadEntry) => (
        <div key={p.dataKey} className="flex items-center gap-2 text-muted-foreground font-light">
          <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: p.fill }} />
          <span className="capitalize">{p.dataKey}</span>
          <span className="ml-auto tabular-nums">{p.value}h</span>
        </div>
      ))}
      <div className="border-t border-border-subtle mt-1 pt-1 flex justify-between text-foreground font-medium">
        <span>Total</span>
        <span className="tabular-nums">{total.toFixed(1)}h</span>
      </div>
    </div>
  );
}
