import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

export const NO_VALUE_GROUP_KEY = "__no_value__";

export interface GroupBucket {
  key: string;
  label: string;
  entities: Entity[];
}

type Option = { value: string; label: string };

function fieldOptions(field: FieldDefinition): Option[] {
  const direct = field.options;
  const fromConfig = (field as unknown as { config?: { options?: unknown } }).config?.options;
  const raw = Array.isArray(direct) ? direct : Array.isArray(fromConfig) ? fromConfig : [];
  return raw.map((o) => {
    if (typeof o === "string") return { value: o, label: o };
    if (o && typeof o === "object") {
      const v = (o as { value?: unknown }).value;
      const l = (o as { label?: unknown }).label;
      const value = typeof v === "string" ? v : String(v ?? "");
      const label = typeof l === "string" ? l : value;
      return { value, label };
    }
    return { value: String(o), label: String(o) };
  });
}

function emptyBucket(): GroupBucket {
  return { key: NO_VALUE_GROUP_KEY, label: "No value", entities: [] };
}

function groupBySelect(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const options = fieldOptions(field);
  const buckets = new Map<string, GroupBucket>();
  for (const opt of options) {
    buckets.set(opt.value, { key: opt.value, label: opt.label, entities: [] });
  }
  const empty = emptyBucket();
  for (const e of entities) {
    const v = e.fields[field.slug];
    if (v === null || v === undefined || v === "") {
      empty.entities.push(e);
      continue;
    }
    const key = String(v);
    let bucket = buckets.get(key);
    if (!bucket) {
      bucket = { key, label: key, entities: [] };
      buckets.set(key, bucket);
    }
    bucket.entities.push(e);
  }
  const ordered = [...buckets.values()];
  if (empty.entities.length > 0) ordered.push(empty);
  return ordered;
}

function groupByMultiSelect(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const options = fieldOptions(field);
  const buckets = new Map<string, GroupBucket>();
  for (const opt of options) {
    buckets.set(opt.value, { key: opt.value, label: opt.label, entities: [] });
  }
  const empty = emptyBucket();
  for (const e of entities) {
    const raw = e.fields[field.slug];
    const values = Array.isArray(raw) ? raw.map(String) : [];
    if (values.length === 0) {
      empty.entities.push(e);
      continue;
    }
    for (const v of values) {
      let bucket = buckets.get(v);
      if (!bucket) {
        bucket = { key: v, label: v, entities: [] };
        buckets.set(v, bucket);
      }
      bucket.entities.push(e);
    }
  }
  const ordered = [...buckets.values()].filter((b) => b.entities.length > 0);
  if (empty.entities.length > 0) ordered.push(empty);
  return ordered;
}

function groupByCheckbox(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const yes: GroupBucket = { key: "true", label: "Checked", entities: [] };
  const no: GroupBucket = { key: "false", label: "Unchecked", entities: [] };
  for (const e of entities) {
    if (e.fields[field.slug] === true) yes.entities.push(e);
    else no.entities.push(e);
  }
  return [yes, no].filter((b) => b.entities.length > 0);
}

export function groupEntities(
  entities: Entity[],
  schema: DatabaseSchema,
  groupBy: string | undefined,
): GroupBucket[] {
  if (!groupBy) return [{ key: "all", label: "", entities }];
  const field = schema.fields.find((f) => f.slug === groupBy);
  if (!field) {
    const bucket = emptyBucket();
    bucket.entities = entities;
    return [bucket];
  }
  switch (field.fieldType) {
    case "multi_select":
      return groupByMultiSelect(entities, field);
    case "checkbox":
      return groupByCheckbox(entities, field);
    default:
      return groupBySelect(entities, field);
  }
}
