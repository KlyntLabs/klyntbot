const RE = /^(\d+)(s|m|h|d)$/i;

export function parseDurationToEndsAt(input: string): string | null {
  const m = input.trim().match(RE);
  if (!m) return null;
  const value = parseInt(m[1], 10);
  const unit = m[2].toLowerCase();
  let ms: number;
  switch (unit) {
    case "s":
      ms = value * 1_000;
      break;
    case "m":
      ms = value * 60_000;
      break;
    case "h":
      ms = value * 3_600_000;
      break;
    case "d":
      ms = value * 86_400_000;
      break;
    default:
      return null;
  }
  return new Date(Date.now() + ms).toISOString();
}
