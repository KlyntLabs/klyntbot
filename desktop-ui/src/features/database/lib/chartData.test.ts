import type { ChartConfig, DatabaseSchema, Entity } from "@shared/types";
import { describe, expect, it } from "vitest";
import { chartData } from "./chartData";

const schema = {
  id: "db1",
  fields: [
    { id: "f1", slug: "status", name: "Status", fieldType: "select", options: ["todo", "done"] },
    {
      id: "f2",
      slug: "tags",
      name: "Tags",
      fieldType: "multi_select",
      options: ["urgent", "home"],
    },
  ],
  views: [],
} as unknown as DatabaseSchema;

const entities: Entity[] = [
  { id: "a", databaseId: "db1", fields: { status: "todo", tags: ["urgent"] } } as unknown as Entity,
  {
    id: "b",
    databaseId: "db1",
    fields: { status: "todo", tags: ["urgent", "home"] },
  } as unknown as Entity,
  { id: "c", databaseId: "db1", fields: { status: "done", tags: [] } } as unknown as Entity,
];

describe("chartData", () => {
  it("counts by select field", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "status", aggregation: "count" };
    const { series, xLabel } = chartData(entities, schema, config);
    expect(xLabel).toBe("Status");
    expect(series).toEqual([
      { x: "todo", y: 2 },
      { x: "done", y: 1 },
    ]);
  });

  it("counts by multi_select fans entities into each value", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "tags", aggregation: "count" };
    const { series } = chartData(entities, schema, config);
    expect(series.find((p) => p.x === "urgent")?.y).toBe(2);
    expect(series.find((p) => p.x === "home")?.y).toBe(1);
  });

  it("returns empty series and falls back to slug when field missing", () => {
    const config: ChartConfig = { chartType: "bar", xAxis: "missing", aggregation: "count" };
    const { series, xLabel } = chartData(entities, schema, config);
    expect(series).toEqual([]);
    expect(xLabel).toBe("missing");
  });

  it("collapses long tail into Other (top 12 + Other)", () => {
    const many: Entity[] = Array.from(
      { length: 20 },
      (_, i) =>
        ({ id: `e${i}`, databaseId: "db1", fields: { status: `s${i}` } }) as unknown as Entity,
    );
    const config: ChartConfig = { chartType: "bar", xAxis: "status", aggregation: "count" };
    const { series } = chartData(many, schema, config);
    expect(series.length).toBe(13);
    expect(series[12]?.x).toBe("Other");
    expect(series[12]?.y).toBe(8);
  });
});
