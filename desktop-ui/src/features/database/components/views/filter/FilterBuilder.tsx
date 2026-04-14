import type {
  DatabaseSchema,
  FieldDefinition,
  FilterGroup,
  FilterNode,
  FilterOp,
  LogicOp,
} from "@shared/types";
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

const MAX_DEPTH = 3;

const OPS_BY_TYPE: Record<string, FilterOp[]> = {
  text: ["contains", "not_contains", "eq", "neq", "is_empty", "is_not_empty"],
  number: ["eq", "neq", "gt", "gte", "lt", "lte", "is_empty", "is_not_empty"],
  select: ["eq", "neq", "in", "not_in", "is_empty", "is_not_empty"],
  multi_select: ["in", "not_in", "is_empty", "is_not_empty"],
  date: ["eq", "neq", "gt", "gte", "lt", "lte", "is_empty", "is_not_empty"],
  checkbox: ["eq"],
  default: ["eq", "neq", "contains", "is_empty", "is_not_empty"],
};

const OP_LABELS: Record<FilterOp, string> = {
  eq: "is",
  neq: "is not",
  gt: ">",
  gte: "≥",
  lt: "<",
  lte: "≤",
  contains: "contains",
  not_contains: "doesn't contain",
  is_empty: "is empty",
  is_not_empty: "is not empty",
  in: "is any of",
  not_in: "is none of",
};

function opsForField(field: FieldDefinition | undefined): FilterOp[] {
  if (!field) return OPS_BY_TYPE.default;
  return OPS_BY_TYPE[field.fieldType] ?? OPS_BY_TYPE.default;
}

function needsValue(op: FilterOp): boolean {
  return !["is_empty", "is_not_empty"].includes(op);
}

function emptyRule(schema: DatabaseSchema): FilterNode {
  const field = schema.fields.find((f) => !f.hidden);
  const ops = opsForField(field);
  return {
    kind: "rule",
    field: field?.slug ?? "",
    op: ops[0],
    value: "",
  };
}

function emptyGroup(): FilterGroup {
  return { op: "and", nodes: [] };
}

function emptyGroupNode(): Extract<FilterNode, { kind: "group" }> {
  return { kind: "group", op: "and", nodes: [] };
}

interface FilterBuilderProps {
  schema: DatabaseSchema;
  value: FilterGroup | undefined;
  onChange: (value: FilterGroup | undefined) => void;
}

export function FilterBuilder({ schema, value, onChange }: FilterBuilderProps) {
  const group = value ?? emptyGroup();
  const isEmpty = group.nodes.length === 0;

  const update = (next: FilterGroup) => {
    onChange(next.nodes.length === 0 ? undefined : next);
  };

  return (
    <div className="space-y-2">
      {!isEmpty && <GroupEditor schema={schema} group={group} onChange={update} depth={0} isRoot />}
      {isEmpty && (
        <button
          type="button"
          onClick={() => update({ ...group, nodes: [emptyRule(schema)] })}
          className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] text-foreground/70 hover:bg-accent hover:text-foreground"
        >
          <Plus className="h-3 w-3" />
          Add filter
        </button>
      )}
    </div>
  );
}

interface GroupEditorProps {
  schema: DatabaseSchema;
  group: FilterGroup;
  onChange: (next: FilterGroup) => void;
  onRemove?: () => void;
  depth: number;
  isRoot?: boolean;
}

function GroupEditor({ schema, group, onChange, onRemove, depth, isRoot }: GroupEditorProps) {
  const canNest = depth < MAX_DEPTH - 1;

  const setOp = (op: LogicOp) => onChange({ ...group, op });
  const addRule = () => onChange({ ...group, nodes: [...group.nodes, emptyRule(schema)] });
  const addGroup = () => onChange({ ...group, nodes: [...group.nodes, emptyGroupNode()] });
  const replaceNode = (i: number, node: FilterNode) =>
    onChange({ ...group, nodes: group.nodes.map((n, j) => (j === i ? node : n)) });
  const removeNode = (i: number) =>
    onChange({ ...group, nodes: group.nodes.filter((_, j) => j !== i) });

  return (
    <div
      className={`space-y-1.5 rounded-md ${isRoot ? "" : "border border-border/60 bg-surface-lowest/50 p-2"}`}
    >
      {group.nodes.map((node, i) => (
        <div key={i} className="flex items-center gap-1.5">
          {i === 0 ? (
            <span className="w-14 shrink-0 text-[11px] text-foreground/55">Where</span>
          ) : i === 1 ? (
            <LogicToggle value={group.op} onChange={setOp} />
          ) : (
            <span className="w-14 shrink-0 text-center text-[11px] uppercase text-foreground/45">
              {group.op}
            </span>
          )}
          {node.kind === "rule" ? (
            <RuleEditor
              schema={schema}
              rule={node}
              onChange={(n) => replaceNode(i, n)}
              onRemove={() => removeNode(i)}
            />
          ) : (
            <div className="flex-1 min-w-0">
              <GroupEditor
                schema={schema}
                group={node}
                onChange={(n) => replaceNode(i, { kind: "group", ...n })}
                onRemove={() => removeNode(i)}
                depth={depth + 1}
              />
            </div>
          )}
        </div>
      ))}
      <div className="flex items-center gap-1 pl-14">
        <button
          type="button"
          onClick={addRule}
          className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-foreground/60 hover:bg-accent hover:text-foreground"
        >
          <Plus className="h-3 w-3" /> Add filter
        </button>
        {canNest && (
          <button
            type="button"
            onClick={addGroup}
            className="flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] text-foreground/60 hover:bg-accent hover:text-foreground"
          >
            <Plus className="h-3 w-3" /> Add group
          </button>
        )}
        {onRemove && (
          <button
            type="button"
            onClick={onRemove}
            className="ml-auto rounded p-0.5 text-foreground/45 hover:bg-red-500/10 hover:text-red-500"
            aria-label="Remove group"
          >
            <Trash2 className="h-3 w-3" />
          </button>
        )}
      </div>
    </div>
  );
}

function LogicToggle({ value, onChange }: { value: LogicOp; onChange: (v: LogicOp) => void }) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as LogicOp)}
      className="w-14 shrink-0 rounded border border-border bg-background px-1 py-0.5 text-[11px] uppercase text-foreground/70 outline-none"
    >
      <option value="and">and</option>
      <option value="or">or</option>
    </select>
  );
}

interface RuleEditorProps {
  schema: DatabaseSchema;
  rule: Extract<FilterNode, { kind: "rule" }>;
  onChange: (rule: FilterNode) => void;
  onRemove: () => void;
}

function RuleEditor({ schema, rule, onChange, onRemove }: RuleEditorProps) {
  const field = schema.fields.find((f) => f.slug === rule.field);
  const ops = opsForField(field);
  const currentOp = ops.includes(rule.op) ? rule.op : ops[0];

  return (
    <div className="flex flex-1 min-w-0 items-center gap-1">
      <select
        value={rule.field}
        onChange={(e) => {
          const newField = schema.fields.find((f) => f.slug === e.target.value);
          const newOps = opsForField(newField);
          onChange({ ...rule, field: e.target.value, op: newOps[0], value: "" });
        }}
        className="max-w-[30%] rounded border border-border bg-background px-1.5 py-0.5 text-[12px] outline-none truncate"
      >
        {schema.fields
          .filter((f) => !f.hidden)
          .map((f) => (
            <option key={f.slug} value={f.slug}>
              {f.name}
            </option>
          ))}
      </select>
      <select
        value={currentOp}
        onChange={(e) => onChange({ ...rule, op: e.target.value as FilterOp })}
        className="rounded border border-border bg-background px-1.5 py-0.5 text-[12px] outline-none"
      >
        {ops.map((op) => (
          <option key={op} value={op}>
            {OP_LABELS[op]}
          </option>
        ))}
      </select>
      {needsValue(currentOp) && (
        <RuleValueInput
          fieldType={field?.fieldType}
          value={rule.value}
          onCommit={(value) => onChange({ ...rule, value })}
        />
      )}
      <button
        type="button"
        onClick={onRemove}
        className="shrink-0 rounded p-0.5 text-foreground/45 hover:bg-red-500/10 hover:text-red-500"
        aria-label="Remove filter"
      >
        <Trash2 className="h-3 w-3" />
      </button>
    </div>
  );
}

/** Buffers keystrokes locally; only commits to parent on blur or Enter so every character
 *  doesn't round-trip through updateView. */
function RuleValueInput({
  fieldType,
  value,
  onCommit,
}: {
  fieldType: FieldDefinition["fieldType"] | undefined;
  value: unknown;
  onCommit: (v: string) => void;
}) {
  const [local, setLocal] = useState(String(value ?? ""));
  useEffect(() => {
    setLocal(String(value ?? ""));
  }, [value]);
  const inputType = fieldType === "number" ? "number" : fieldType === "date" ? "date" : "text";
  return (
    <input
      type={inputType}
      value={local}
      onChange={(e) => setLocal(e.target.value)}
      onBlur={() => {
        if (local !== String(value ?? "")) onCommit(local);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
      }}
      placeholder="Value"
      className="flex-1 min-w-0 rounded border border-border bg-background px-1.5 py-0.5 text-[12px] outline-none"
    />
  );
}
