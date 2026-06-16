// Mirrors crates/desktop-shared/src/types.rs#EntityKind. Strings match the
// backend's serde camelCase encoding so we can compare directly against
// EntityUpdatedPayload.entityKind without translation.
export type EntityKind =
  | "task"
  | "project"
  | "objective"
  | "area"
  | "keyResult"
  | "focusSession"
  | "productivity"
  | "note"
  | "notebook"
  | "finance"
  | "source"
  | "conversation"
  | "mirrorSnippet"
  | "brainVersion"
  | "pendingMemory";

// Ordered longest-prefix-first so "notebook_" wins over "note_".
const PREFIX_TABLE: ReadonlyArray<readonly [string, EntityKind]> = [
  ["notebook_", "notebook"],
  ["note_", "note"],
  ["task_", "task"],
  ["project_", "project"],
  ["objective_", "objective"],
  ["area_", "area"],
  ["key_result_", "keyResult"],
  ["focus_", "focusSession"],
  ["productivity_", "productivity"],
  ["finance_", "finance"],
  ["source_", "source"],
  ["conversation_", "conversation"],
];

export function entityKindForCommand(cmd: string): EntityKind | null {
  for (const [prefix, kind] of PREFIX_TABLE) {
    if (cmd.startsWith(prefix)) return kind;
  }
  return null;
}
