import type { BufferGeometry, MeshStandardMaterial } from "three";
import {
  createEntityGeometry,
  createEntityMaterial,
  createFinanceGeometry,
  createFinanceMaterial,
  createLearningGeometry,
  createLearningMaterial,
  createNodeGeometry,
  createNodeMaterial,
  createOkrGeometry,
  createOkrMaterial,
  createProductivityGeometry,
  createProductivityMaterial,
  createProjectGeometry,
  createProjectMaterial,
  createTreeMaterial,
} from "./graphMaterials";

function bucketSize(value: number, step: number): number {
  return Math.round(value / step) * step;
}

const geometryCache = new Map<string, BufferGeometry>();
const materialCache = new Map<string, MeshStandardMaterial>();

export function getPooledGeometry(type: string, size: number, linkCount: number): BufferGeometry {
  const bucketed = bucketSize(size, 2);
  const key = `${type}:${bucketed}:${bucketSize(linkCount, 3)}`;

  let geo = geometryCache.get(key);
  if (geo) return geo;

  switch (type) {
    case "entity":
      geo = createEntityGeometry(linkCount);
      break;
    case "tree_section": {
      const radius = 3 + (Math.min(linkCount, 5) / 5) * 2;
      geo = createNodeGeometry(radius * 2);
      break;
    }
    case "tree_text":
      geo = createNodeGeometry(3);
      break;
    case "finance":
      geo = createFinanceGeometry(bucketed);
      break;
    case "productivity":
      geo = createProductivityGeometry(bucketed);
      break;
    case "okr":
      geo = createOkrGeometry(bucketed);
      break;
    case "learning":
      geo = createLearningGeometry(bucketed);
      break;
    case "project":
      geo = createProjectGeometry(bucketed);
      break;
    default:
      geo = createNodeGeometry(bucketed * 0.3);
      break;
  }

  geometryCache.set(key, geo);
  return geo;
}

export function getPooledMaterial(
  type: string,
  color: string,
  emissiveIntensity: number,
): MeshStandardMaterial {
  const key = `${type}:${color}:${emissiveIntensity.toFixed(1)}`;

  let mat = materialCache.get(key);
  if (mat) return mat;

  switch (type) {
    case "entity":
      mat = createEntityMaterial(color);
      break;
    case "tree_section":
      mat = createTreeMaterial(color, 0.6);
      break;
    case "tree_text":
      mat = createTreeMaterial(color, 0.3);
      break;
    case "finance":
      mat = createFinanceMaterial(color);
      break;
    case "productivity":
      mat = createProductivityMaterial(color);
      break;
    case "okr":
      mat = createOkrMaterial(color);
      break;
    case "learning":
      mat = createLearningMaterial(color);
      break;
    case "project":
      mat = createProjectMaterial(color);
      break;
    default:
      mat = createNodeMaterial(color, emissiveIntensity);
      break;
  }

  mat.userData = { baseEmissive: emissiveIntensity };
  materialCache.set(key, mat);
  return mat;
}

export function disposePool(): void {
  for (const geo of geometryCache.values()) geo.dispose();
  for (const mat of materialCache.values()) mat.dispose();
  geometryCache.clear();
  materialCache.clear();
}
