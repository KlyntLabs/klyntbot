export function formatDownloadSize(bytes: number | null | undefined) {
  if (!bytes || bytes <= 0) {
    return "0 MB";
  }
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) {
    const digits = gb >= 10 ? 0 : 1;
    return `${gb.toFixed(digits)} GB`;
  }
  const mb = bytes / 1024 ** 2;
  const digits = mb >= 10 ? 0 : 1;
  return `${mb.toFixed(digits)} MB`;
}

/** Format a byte count as human-readable B/KB/MB/GB. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 ** 3) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / 1024 ** 3).toFixed(1)} GB`;
}
