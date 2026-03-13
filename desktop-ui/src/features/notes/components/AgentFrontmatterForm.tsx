import { ChevronDown, ChevronRight, X } from "lucide-react";
import { memo, useCallback, useEffect, useRef, useState } from "react";

// ── YAML Field Types ──────────────────────────────────────────────────

type FieldValue =
  | { type: "string"; value: string }
  | { type: "number"; value: number }
  | { type: "array"; value: string[] };

interface ParsedField {
  key: string;
  raw: string;
  parsed: FieldValue;
}

function parseYamlValue(raw: string): FieldValue {
  const trimmed = raw.trim();
  if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
    const inner = trimmed.slice(1, -1).trim();
    if (!inner) return { type: "array", value: [] };
    const items = inner.split(",").map((s) => s.trim().replace(/^["']|["']$/g, ""));
    return { type: "array", value: items.filter(Boolean) };
  }
  if (/^\d+$/.test(trimmed)) {
    return { type: "number", value: Number.parseInt(trimmed, 10) };
  }
  return { type: "string", value: trimmed };
}

function parseYamlFields(yaml: string): ParsedField[] {
  const fields: ParsedField[] = [];
  for (const line of yaml.split("\n")) {
    const m = line.match(/^(\w[\w_]*)\s*:\s*(.*)$/);
    if (m) {
      fields.push({ key: m[1], raw: m[2].trim(), parsed: parseYamlValue(m[2]) });
    }
  }
  return fields;
}

function serializeFieldValue(val: FieldValue): string {
  switch (val.type) {
    case "string":
      return val.value;
    case "number":
      return String(val.value);
    case "array": {
      if (val.value.length === 0) return "[]";
      const items = val.value.map((v) => (/[,["'\] ]/.test(v) ? `"${v}"` : v));
      return `[${items.join(", ")}]`;
    }
  }
}

function serializeFields(fields: ParsedField[]): string {
  return fields.map((f) => `${f.key}: ${serializeFieldValue(f.parsed)}`).join("\n");
}

// ── Tag Input ─────────────────────────────────────────────────────────

function TagInput({
  values,
  onChange,
  placeholder,
}: {
  values: string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
}) {
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  const addTag = useCallback(
    (tag: string) => {
      const trimmed = tag.trim();
      if (trimmed && !values.includes(trimmed)) {
        onChange([...values, trimmed]);
      }
      setInput("");
    },
    [values, onChange],
  );

  const removeTag = useCallback(
    (index: number) => {
      onChange(values.filter((_, i) => i !== index));
    },
    [values, onChange],
  );

  return (
    <fieldset
      className="flex flex-wrap items-center gap-1 min-h-[24px] px-1.5 py-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] focus-within:border-brand/40 transition-colors cursor-text"
      onClick={() => inputRef.current?.focus()}
      onKeyDown={() => inputRef.current?.focus()}
    >
      {values.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-0.5 px-1.5 py-0 rounded bg-white/[0.08] text-[11px] text-secondary font-mono"
        >
          {tag}
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              removeTag(values.indexOf(tag));
            }}
            className="text-dim hover:text-primary transition-colors"
          >
            <X size={8} />
          </button>
        </span>
      ))}
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          if ((e.key === "Enter" || e.key === ",") && input.trim()) {
            e.preventDefault();
            addTag(input);
          }
          if (e.key === "Backspace" && !input && values.length > 0) {
            removeTag(values.length - 1);
          }
        }}
        onBlur={() => {
          if (input.trim()) addTag(input);
        }}
        placeholder={values.length === 0 ? (placeholder ?? "Add...") : ""}
        className="flex-1 min-w-[60px] bg-transparent text-[11px] text-primary font-mono outline-none placeholder:text-dim/40"
      />
    </fieldset>
  );
}

// ── Main Form ─────────────────────────────────────────────────────────

interface AgentFrontmatterFormProps {
  frontmatter: string;
  onChange: (frontmatter: string) => void;
}

export const AgentFrontmatterForm = memo(function AgentFrontmatterForm({
  frontmatter,
  onChange,
}: AgentFrontmatterFormProps) {
  const [fields, setFields] = useState<ParsedField[]>(() => parseYamlFields(frontmatter));
  const [expanded, setExpanded] = useState(false);

  // Sync if frontmatter prop changes (e.g. switching files)
  useEffect(() => {
    setFields(parseYamlFields(frontmatter));
  }, [frontmatter]);

  const updateField = useCallback(
    (index: number, newValue: FieldValue) => {
      setFields((prev) => {
        const next = prev.map((f, i) => (i === index ? { ...f, parsed: newValue } : f));
        onChange(serializeFields(next));
        return next;
      });
    },
    [onChange],
  );

  if (fields.length === 0) return null;

  const nameField = fields.find((f) => f.key === "name");
  const descField = fields.find((f) => f.key === "description");
  const extraFields = fields.filter((f) => f.key !== "name" && f.key !== "description");

  return (
    <div className="mx-3 mt-2 rounded-lg bg-white/[0.03] border border-white/[0.06] text-xs">
      {/* Name + Description row */}
      <div className="flex items-center gap-2 px-3 py-1.5">
        {nameField && (
          <input
            type="text"
            value={nameField.parsed.value as string}
            onChange={(e) => {
              const idx = fields.indexOf(nameField);
              updateField(idx, { type: "string", value: e.target.value });
            }}
            className="font-medium text-secondary bg-transparent border-b border-transparent hover:border-white/[0.1] focus:border-brand/40 outline-none px-0.5 max-w-[120px] font-mono"
          />
        )}
        {descField && (
          <input
            type="text"
            value={descField.parsed.value as string}
            onChange={(e) => {
              const idx = fields.indexOf(descField);
              updateField(idx, { type: "string", value: e.target.value });
            }}
            className="flex-1 text-dim bg-transparent border-b border-transparent hover:border-white/[0.1] focus:border-brand/40 outline-none px-0.5 truncate"
          />
        )}
        {extraFields.length > 0 && (
          <button
            type="button"
            onClick={() => setExpanded((p) => !p)}
            className="text-dim hover:text-secondary transition-colors shrink-0"
          >
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </button>
        )}
      </div>

      {/* Extra fields */}
      {expanded && extraFields.length > 0 && (
        <div className="px-3 pb-2 pt-1 border-t border-white/[0.04] flex flex-col gap-1.5">
          {extraFields.map((field) => {
            const idx = fields.indexOf(field);
            return (
              <div key={field.key} className="flex items-start gap-3">
                <span className="text-dim font-mono w-[130px] shrink-0 pt-0.5 text-right">
                  {field.key}
                </span>
                <div className="flex-1 min-w-0">
                  <FieldEditor field={field} onUpdate={(val) => updateField(idx, val)} />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
});

// ── Field Editor (dispatches by type) ─────────────────────────────────

function FieldEditor({
  field,
  onUpdate,
}: {
  field: ParsedField;
  onUpdate: (val: FieldValue) => void;
}) {
  const { parsed } = field;

  if (parsed.type === "array") {
    return (
      <TagInput
        values={parsed.value}
        onChange={(values) => onUpdate({ type: "array", value: values })}
        placeholder={`Add ${field.key}...`}
      />
    );
  }

  if (parsed.type === "number") {
    return (
      <input
        type="number"
        value={parsed.value}
        onChange={(e) =>
          onUpdate({ type: "number", value: Number.parseInt(e.target.value, 10) || 0 })
        }
        className="w-[80px] px-1.5 py-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] focus:border-brand/40 outline-none text-[11px] text-primary font-mono"
      />
    );
  }

  return (
    <input
      type="text"
      value={parsed.value}
      onChange={(e) => onUpdate({ type: "string", value: e.target.value })}
      className="w-full px-1.5 py-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] focus:border-brand/40 outline-none text-[11px] text-primary font-mono"
    />
  );
}
