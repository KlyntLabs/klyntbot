import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { DatabaseSchema, FieldDefinition, FieldType } from "@shared/types";
import { Button } from "@shared/ui/Button";
import { Input } from "@shared/ui/Input";
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
      databaseId: schema.id,
      name: newFieldName,
      fieldType: newFieldType,
    });
    setNewFieldName("");
    setAddingField(false);
    emitDatabaseUpdated();
  };

  const handleRemoveField = async (fieldId: string) => {
    await removeField({ databaseId: schema.id, fieldId });
    emitDatabaseUpdated();
  };

  return (
    <div className="glass-card ml-2 mr-2 mb-2 w-80 shrink-0 flex flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border/60 px-4 py-2 shrink-0">
        <h3 className="text-[13px] font-semibold text-foreground">Properties</h3>
        <button
          type="button"
          onClick={onClose}
          className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors cursor-pointer"
        >
          <svg
            className="h-4 w-4"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            strokeWidth={1.5}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      <div className="flex-1 overflow-y-auto space-y-0.5 p-3">
        {schema.fields.map((field) => (
          <div
            key={field.id}
            className="group flex items-center gap-2 rounded-lg px-2.5 py-2 hover:bg-accent transition-colors"
          >
            <span className="w-5 shrink-0 text-center text-[13px] text-dim">
              {fieldTypeIcon(field.fieldType)}
            </span>
            <span className="flex-1 truncate text-[13px] text-foreground">{field.name}</span>
            {!field.aiManaged && (
              <button
                type="button"
                onClick={() => handleRemoveField(field.id)}
                className="rounded p-0.5 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100 cursor-pointer"
              >
                <svg
                  className="h-3.5 w-3.5"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                  strokeWidth={1.5}
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
                </svg>
              </button>
            )}
          </div>
        ))}
      </div>
      {addingField ? (
        <div className="space-y-2.5 border-t border-border p-3">
          <Input
            value={newFieldName}
            onChange={(e) => setNewFieldName(e.target.value)}
            placeholder="Field name"
            className="w-full"
          />
          <FieldTypeSelector value={newFieldType} onChange={setNewFieldType} />
          <div className="flex gap-2">
            <Button
              variant="primary"
              size="sm"
              onClick={handleAddField}
              loading={loading}
              disabled={!newFieldName.trim()}
            >
              Add
            </Button>
            <Button variant="ghost" size="sm" onClick={() => setAddingField(false)}>
              Cancel
            </Button>
          </div>
        </div>
      ) : (
        <div className="border-t border-border p-3">
          <button
            type="button"
            onClick={() => setAddingField(true)}
            className="flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[13px] text-muted-foreground hover:bg-accent hover:text-foreground transition-colors cursor-pointer"
          >
            <svg
              className="h-3.5 w-3.5"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              strokeWidth={1.5}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
            Add property
          </button>
        </div>
      )}
    </div>
  );
}

function fieldTypeIcon(ft: FieldType): string {
  const icons: Record<FieldType, string> = {
    text: "Aa",
    number: "#",
    select: "▾",
    multi_select: "⊞",
    date: "◷",
    checkbox: "☑",
    url: "↗",
    email: "✉",
    phone: "☏",
    relation: "↔",
    rollup: "Σ",
    formula: "ƒ",
    created_time: "◷",
    last_edited: "✎",
    files: "⎘",
    person: "⊙",
  };
  return icons[ft] ?? ft;
}
