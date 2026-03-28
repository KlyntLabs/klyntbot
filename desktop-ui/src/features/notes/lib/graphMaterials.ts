import { Color, LineBasicMaterial, MeshStandardMaterial, SphereGeometry } from "three";

export function createNodeMaterial(
  hexColor: string,
  emissiveIntensity: number,
): MeshStandardMaterial {
  const color = new Color(hexColor);
  return new MeshStandardMaterial({
    color,
    emissive: color,
    emissiveIntensity,
    transparent: true,
    opacity: 0.9,
    roughness: 0.4,
    metalness: 0.1,
  });
}

export function createNodeGeometry(size: number): SphereGeometry {
  const radius = size / 2;
  const segments = radius > 15 ? 24 : radius > 8 ? 16 : 12;
  return new SphereGeometry(radius, segments, segments);
}

export function createLinkMaterial(hexColor: string, opacity: number): LineBasicMaterial {
  return new LineBasicMaterial({
    color: new Color(hexColor),
    transparent: true,
    opacity: Math.min(opacity, 1),
  });
}
