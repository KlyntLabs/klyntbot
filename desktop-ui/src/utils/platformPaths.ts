type PlatformKind = "mac" | "unknown";

function platformKind(): PlatformKind {
  if (typeof navigator === "undefined") {
    return "unknown";
  }
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.platform ??
    "";
  return platform.toLowerCase().includes("mac") ? "mac" : "unknown";
}

export function isMacPlatform(): boolean {
  return platformKind() === "mac";
}

export function isMobilePlatform(): boolean {
  if (typeof navigator === "undefined") {
    return false;
  }
  const platform =
    (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ??
    navigator.platform ??
    "";
  const normalizedPlatform = platform.toLowerCase();
  const userAgent = (navigator.userAgent ?? "").toLowerCase();
  const maxTouchPoints =
    typeof (navigator as Navigator).maxTouchPoints === "number"
      ? (navigator as Navigator).maxTouchPoints
      : 0;
  const hasTouch = maxTouchPoints > 0;
  const hasMobileUserAgentToken =
    userAgent.includes("mobile") ||
    userAgent.includes("iphone") ||
    userAgent.includes("ipad") ||
    userAgent.includes("ipod") ||
    userAgent.includes("android");
  const iPadDesktopMode =
    normalizedPlatform.includes("mac") &&
    hasTouch &&
    (hasMobileUserAgentToken || userAgent.includes("like mac os x"));
  return (
    normalizedPlatform.includes("iphone") ||
    normalizedPlatform.includes("ipad") ||
    normalizedPlatform.includes("android") ||
    hasMobileUserAgentToken ||
    iPadDesktopMode
  );
}

export function fileManagerName(): string {
  return "Finder";
}

export function revealInFileManagerLabel(): string {
  return "Reveal in Finder";
}

export function openInFileManagerLabel(): string {
  return "Open in Finder";
}

export function isAbsolutePath(value: string): boolean {
  const trimmed = value.trim();
  return Boolean(trimmed) && (trimmed.startsWith("/") || trimmed.startsWith("~/"));
}

function stripTrailingSeparators(value: string) {
  return value.replace(/[/]+$/, "");
}

function stripLeadingSeparators(value: string) {
  return value.replace(/^[/]+/, "");
}

export function joinWorkspacePath(base: string, path: string): string {
  const trimmedBase = base.trim();
  const trimmedPath = path.trim();
  if (!trimmedBase) {
    return trimmedPath;
  }
  if (!trimmedPath || isAbsolutePath(trimmedPath)) {
    return trimmedPath;
  }

  const baseWithoutTrailing = stripTrailingSeparators(trimmedBase);
  const pathWithoutLeading = stripLeadingSeparators(trimmedPath);
  return `${baseWithoutTrailing}/${pathWithoutLeading}`;
}
