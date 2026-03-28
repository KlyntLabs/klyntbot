/**
 * Shared utilities for the graph rendering layer.
 */

// Cache parsed RGB triples so we only parse each hex string once.
// Only the alpha varies per call, so caching the [r, g, b] is sufficient.
const rgbCache = new Map<string, [number, number, number]>();

function parseRgb(hex: string): [number, number, number] {
  const cached = rgbCache.get(hex);
  if (cached) return cached;

  const r = Number.parseInt(hex.slice(1, 3), 16);
  const g = Number.parseInt(hex.slice(3, 5), 16);
  const b = Number.parseInt(hex.slice(5, 7), 16);
  const triple: [number, number, number] = [r, g, b];
  rgbCache.set(hex, triple);
  return triple;
}

/**
 * Convert a hex color string to an rgba() CSS value.
 * Handles both 6-digit (#RRGGBB) and 8-digit (#RRGGBBAA) hex strings.
 */
export function hexToRgba(hex: string, alpha: number): string {
  const [r, g, b] = parseRgb(hex);

  if (hex.length === 9) {
    const hexAlpha = Number.parseInt(hex.slice(7, 9), 16) / 255;
    return `rgba(${r},${g},${b},${hexAlpha * alpha})`;
  }

  return `rgba(${r},${g},${b},${alpha})`;
}

/**
 * Extract the string ID from a force-graph link endpoint.
 *
 * react-force-graph mutates link.source/target from string IDs to node object
 * references after simulation starts. This helper normalizes both forms.
 */
export function resolveLinkEndpointId(endpoint: string | { id: string }): string {
  return typeof endpoint === "string" ? endpoint : endpoint.id;
}
