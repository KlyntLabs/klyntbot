import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { DatabaseSchema, FieldDefinition, FieldType } from "@shared/types";
import { useState } from "react";
import { FieldTypeSelector } from "./FieldTypeSelector";

interface SchemaEditorProps {
  schema: DatabaseSchema;
  onClose: () => void;
}

export function SchemaEditor({ schema, onClose }: SchemaEditorProps) {
  const [addingField, setAddingField] = useState(false);
  const [newFieldName, setNewFieldName] = useState("");
  const [newFieldType, setNewFieldType] = useState<FieldType>("text");

  const { mutate: addField, loading } = useMutation<FieldDefinition, Record<string, unknown>>(
    "db_add_field",
  );
  const { mutate: removeField } = useMutation<void, Record<string, unknown>>("db_remove_field");

  const handleAddField = async () => {
    if (!newFieldName.trim()) return;
    await addField({
      database_id: schema.id,
      input: { name: newFieldName, field_type: newFieldType },
    });
    setNewFieldName("");
    setAddingField(false);
    emitDatabaseUpdated();
  };

  const handleRemoveField = async (fieldId: string) => {
    await removeField({ database_id: schema.id, field_id: fieldId });
    emitDatabaseUpdated();
  };

  return (
    <div className="h-full w-80 overflow-y-auto border-l border-border bg-surface-base">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <h3 className="text-sm font-semibold">Properties</h3>
        <button type="button" onClick={onClose} className="text-muted hover:text-foreground">
          \u2715
        </button>
      </div>
      <div className="space-y-1 p-3">
        {schema.fields.map((field) => (
          <div
            key={field.id}
            className="group flex items-center gap-2 rounded px-2 py-1.5 hover:bg-surface-hover"
          >
            <span className="w-16 truncate text-xs text-muted">
              {fieldTypeLabel(field.fieldType)}
            </span>
            <span className="flex-1 truncate text-sm">{field.name}</span>
            {!field.aiManaged && (
              <button
                type="button"
                onClick={() => handleRemoveField(field.id)}
                className="text-xs text-red-400 opacity-0 group-hover:opacity-100"
              >
                \u2715
              </button>
            )}
          </div>
        ))}
      </div>
      {addingField ? (
        <div className="space-y-2 border-t border-border p-3">
          <input
            type="text"
            value={newFieldName}
            onChange={(e) => setNewFieldName(e.target.value)}
            placeholder="Field name"
            className="w-full rounded border border-border bg-surface-base px-2 py-1 text-sm outline-none focus:border-accent"
          />
          <FieldTypeSelector value={newFieldType} onChange={setNewFieldType} />
          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleAddField}
              disabled={loading || !newFieldName.trim()}
              className="rounded bg-accent px-3 py-1 text-xs text-white hover:bg-accent/90 disabled:opacity-50"
            >
              Add
            </button>
            <button
              type="button"
              onClick={() => setAddingField(false)}
              className="rounded px-3 py-1 text-xs text-muted hover:bg-surface-hover"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="border-t border-border p-3">
          <button
            type="button"
            onClick={() => setAddingField(true)}
            className="w-full rounded px-3 py-1.5 text-left text-sm text-muted hover:bg-surface-hover"
          >
            + Add property
          </button>
        </div>
      )}
    </div>
  );
}

function fieldTypeLabel(ft: FieldType): string {
  const labels: Record<FieldType, string> = {
    text: "Aa",
    number: "#",
    select: "\u25BE",
    multi_select: "\u229E",
    date: "\uD83D\uDCC5",
    checkbox: "\u2611",
    url: "\uD83D\uDD17",
    email: "\u2709",
    phone: "\uD83D\uDCDE",
    relation: "\u2194",
    rollup: "\u03A3",
    formula: "\u0192",
    created_time: "\u23F1",
    last_edited: "\u270E",
    files: "\uD83D\uDCCE",
    person: "\uD83D\uDC64",
  };
  return labels[ft] ?? ft;
}
