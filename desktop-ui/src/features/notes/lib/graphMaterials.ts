import {
  BoxGeometry,
  CanvasTexture,
  Color,
  CylinderGeometry,
  DodecahedronGeometry,
  LineBasicMaterial,
  MeshStandardMaterial,
  OctahedronGeometry,
  SphereGeometry,
  Sprite,
  SpriteMaterial,
  TorusGeometry,
} from "three";

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

/** Entity nodes — amber/gold diamond shape. */
export function createEntityMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  const emissive = new Color("#f59e0b");
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive,
    emissiveIntensity: 0.6,
    transparent: true,
    opacity: 0.9,
    roughness: 0.3,
    metalness: 0.3,
  });
}

/** OctahedronGeometry for entity nodes. Radius scales from 4–8 with linkCount. */
export function createEntityGeometry(linkCount: number): OctahedronGeometry {
  const normalized = Math.min(linkCount, 20) / 20;
  const radius = 4 + normalized * 4;
  return new OctahedronGeometry(radius);
}

/** Tree section / tree text nodes — semi-transparent sphere in the parent note colour. */
export function createTreeMaterial(color: string, opacity: number): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.2,
    transparent: true,
    opacity: Math.max(0, Math.min(1, opacity)),
    roughness: 0.5,
    metalness: 0.0,
  });
}

/** Finance nodes — DodecahedronGeometry (many-faced, represents value/exchange). */
export function createFinanceGeometry(size: number): DodecahedronGeometry {
  const radius = (size / 2) * 0.3;
  return new DodecahedronGeometry(Math.max(radius, 3));
}

export function createFinanceMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.55,
    transparent: true,
    opacity: 0.9,
    roughness: 0.3,
    metalness: 0.4,
  });
}

/** Productivity nodes — TorusGeometry (ring/donut, represents time cycles). */
export function createProductivityGeometry(size: number): TorusGeometry {
  const outerRadius = Math.max((size / 2) * 0.3, 4);
  return new TorusGeometry(outerRadius, outerRadius * 0.35, 8, 32);
}

export function createProductivityMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.5,
    transparent: true,
    opacity: 0.9,
    roughness: 0.35,
    metalness: 0.2,
  });
}

/** OKR nodes — nested spheres (bullseye, represents goals). Uses CylinderGeometry as flat disc. */
export function createOkrGeometry(size: number): CylinderGeometry {
  const radius = Math.max((size / 2) * 0.3, 4);
  return new CylinderGeometry(radius, radius, radius * 0.3, 32);
}

export function createOkrMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.6,
    transparent: true,
    opacity: 0.9,
    roughness: 0.3,
    metalness: 0.1,
  });
}

/** Learning nodes — BoxGeometry (cube/book shape, represents knowledge). */
export function createLearningGeometry(size: number): BoxGeometry {
  const s = Math.max((size / 2) * 0.3, 3);
  return new BoxGeometry(s * 1.4, s, s * 0.6);
}

export function createLearningMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.5,
    transparent: true,
    opacity: 0.9,
    roughness: 0.4,
    metalness: 0.1,
  });
}

/** Project nodes — CylinderGeometry with 5 segments (pentagon prism). */
export function createProjectGeometry(size: number): CylinderGeometry {
  const radius = Math.max((size / 2) * 0.3, 4);
  return new CylinderGeometry(radius, radius, radius * 0.5, 5);
}

export function createProjectMaterial(color: string): MeshStandardMaterial {
  const nodeColor = new Color(color);
  return new MeshStandardMaterial({
    color: nodeColor,
    emissive: nodeColor,
    emissiveIntensity: 0.55,
    transparent: true,
    opacity: 0.9,
    roughness: 0.35,
    metalness: 0.25,
  });
}

/** Community label — a billboard Sprite rendered from a Canvas. */
export function createLabelSprite(text: string, color: string): Sprite {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    // Fallback: return an empty sprite
    return new Sprite(new SpriteMaterial({ transparent: true, opacity: 0 }));
  }

  const fontSize = 18;
  const padding = 12;

  ctx.font = `bold ${fontSize}px Inter, system-ui, sans-serif`;
  const measured = ctx.measureText(text);
  const textWidth = measured.width;

  canvas.width = textWidth + padding * 2;
  canvas.height = fontSize + padding * 2;

  // Re-apply font after resize (canvas reset clears it)
  ctx.font = `bold ${fontSize}px Inter, system-ui, sans-serif`;
  ctx.textBaseline = "middle";

  // Background pill
  const bgColor = `${color}33`; // 20% opacity
  ctx.fillStyle = bgColor;
  const r = (canvas.height / 2) * 0.9;
  ctx.beginPath();
  ctx.roundRect(0, 0, canvas.width, canvas.height, r);
  ctx.fill();

  // Border
  ctx.strokeStyle = `${color}99`;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.roundRect(0, 0, canvas.width, canvas.height, r);
  ctx.stroke();

  // Text
  ctx.fillStyle = color;
  ctx.fillText(text, padding, canvas.height / 2);

  const texture = new CanvasTexture(canvas);
  const material = new SpriteMaterial({
    map: texture,
    transparent: true,
    opacity: 0.92,
    depthWrite: false,
  });

  const sprite = new Sprite(material);
  // Scale sprite proportionally — canvas aspect determines width vs height
  const aspect = canvas.width / canvas.height;
  sprite.scale.set(aspect * 20, 20, 1);

  return sprite;
}
