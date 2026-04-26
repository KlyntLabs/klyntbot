import { describe, expect, it } from "vitest";
import { type EntityKind, entityKindForCommand } from "../entityKindMap";

describe("entityKindForCommand", () => {
  it.each<[string, EntityKind | null]>([
    ["task_create", "task"],
    ["task_toggle_complete", "task"],
    ["project_archive", "project"],
    ["note_update", "note"],
    ["notebook_create", "notebook"],
    ["finance_transaction_add", "finance"],
    ["focus_session_start", "focusSession"],
    ["coding_memory_recall_fetch", "codingFact"],
    ["coding_memory_distill_now", "codingFact"],
    ["unknown_cmd", null],
    ["", null],
  ])("maps %s -> %s", (cmd, kind) => {
    expect(entityKindForCommand(cmd)).toBe(kind);
  });
});
