import type { DatabaseSchema, Entity } from "@shared/types";
import { describe, expect, it } from "vitest";
import { groupEntities, NO_VALUE_GROUP_KEY } from "./grouping";

const schema = {
  id: "db1",
  name: "Tasks",
  fields: [
    {
      id: "f1",
      slug: "status",
      name: "Status",
      fieldType: "select",
      config: {
        options: [
          { id: "o1", value: "todo", label: "Todo" },
          { id: "o2", value: "done", label: "Done" },
        ],
      },
    },
    {
      id: "f2",
      slug: "tags",
      name: "Tags",
      fieldType: "multi_select",
      config: {
        options: [
          { id: "t1", value: "urgent", label: "Urgent" },
          { id: "t2", value: "home", label: "Home" },
        ],
      },
    },
  ],
  views: [],
} as unknown as DatabaseSchema;

const entities: Entity[] = [
  { id: "a", databaseId: "db1", fields: { status: "todo", tags: ["urgent"] } } as unknown as Entity,
  {
    id: "b",
    databaseId: "db1",
    fields: { status: "done", tags: ["urgent", "home"] },
  } as unknown as Entity,
  { id: "c", databaseId: "db1", fields: { status: null } } as unknown as Entity,
];

describe("groupEntities", () => {
  it("groups by select in option order with No value last", () => {
    const groups = groupEntities(entities, schema, "status");
    expect(groups.map((g) => g.key)).toEqual(["todo", "done", NO_VALUE_GROUP_KEY]);
    expect(groups.map((g) => g.entities.map((e) => e.id))).toEqual([["a"], ["b"], ["c"]]);
  });

  it("fans entities across every multi_select value", () => {
    const groups = groupEntities(entities, schema, "tags");
    const urgent = groups.find((g) => g.key === "urgent");
    const home = groups.find((g) => g.key === "home");
    expect(urgent?.entities.map((e) => e.id).sort()).toEqual(["a", "b"]);
    expect(home?.entities.map((e) => e.id)).toEqual(["b"]);
  });

  it("groups by select where options is a string[] (real shape)", () => {
    const stringSchema = {
      id: "db2",
      name: "T",
      fields: [
        {
          id: "f1",
          slug: "status",
          name: "Status",
          fieldType: "select",
          options: ["todo", "done"],
        },
      ],
      views: [],
    } as unknown as DatabaseSchema;
    const groups = groupEntities(entities, stringSchema, "status");
    expect(groups.map((g) => g.key)).toEqual(["todo", "done", NO_VALUE_GROUP_KEY]);
  });

  it("returns a single No value group when field missing", () => {
    const groups = groupEntities(entities, schema, "nonexistent");
    expect(groups).toHaveLength(1);
    expect(groups[0]?.key).toBe(NO_VALUE_GROUP_KEY);
    expect(groups[0]?.entities).toHaveLength(3);
  });
});
