const TAG_PALETTE = [
  "#a78bfa", // violet
  "#93c5fd", // blue
  "#6ee7b7", // green
  "#fcd34d", // amber
  "#fca5a5", // red
  "#f9a8d4", // pink
  "#a5b4fc", // indigo
  "#67e8f9", // cyan
  "#fdba74", // orange
  "#86efac", // emerald
  "#c4b5fd", // purple
  "#fde68a", // yellow
];

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash |= 0;
  }
  return Math.abs(hash);
}

export function tagColor(tagName: string): string {
  return TAG_PALETTE[hashString(tagName) % TAG_PALETTE.length];
}

export function tagBgColor(tagName: string): string {
  return `${tagColor(tagName)}25`;
}
