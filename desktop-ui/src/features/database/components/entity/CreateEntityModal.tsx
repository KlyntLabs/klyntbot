import { FieldEditor } from "@features/database/components/fields/FieldEditor";
import { useCreateEntity } from "@features/database/hooks/useEntity";
import type { DatabaseSchema } from "@shared/types";
import { useState } from "react";

interface CreateEntityModalProps {
  schema: DatabaseSchema;
  onClose: () => void;
  onCreated?: () => void;
}

export function CreateEntityModal({ schema, onClose, onCreated }: CreateEntityModalProps) {
  const editableFields = schema.fields.filter(
    (f) => !f.hidden && f.fieldType !== "created_time" && f.fieldType !== "last_edited",
  );

  const [fields, setFields] = useState<Record<string, unknown>>(() => {
    const initial: Record<string, unknown> = {};
    for (const f of editableFields) {
      if (f.defaultValue != null) initial[f.slug] = f.defaultValue;
      else if (f.fieldType === "checkbox") initial[f.slug] = false;
    }
    return initial;
  });

  const { mutate: createEntity, loading, error } = useCreateEntity(schema.id);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const result = await createEntity({ fields });
    if (result) {
      onCreated?.();
      onClose();
    }
  };

  return (
    <div
      role="presentation"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onClose}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div
        role="dialog"
        className="w-full max-w-md rounded-lg bg-surface-base border border-border shadow-xl"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-lg font-semibold">New {schema.name}</h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded p-1 text-muted hover:bg-surface-hover"
          >
            {"\u2715"}
          </button>
        </div>
        <form onSubmit={handleSubmit} className="p-4 space-y-4">
          {editableFields.map((field) => (
            <div key={field.id} className="space-y-1">
              <span className="text-sm font-medium">
                {field.name}
                {field.required && <span className="text-red-400 ml-0.5">*</span>}
              </span>
              <FieldEditor
                field={field}
                value={fields[field.slug]}
                onChange={(v) => setFields((prev) => ({ ...prev, [field.slug]: v }))}
              />
            </div>
          ))}
          {error && <p className="text-sm text-red-400">{error.message}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded px-4 py-2 text-sm text-muted hover:bg-surface-hover"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading}
              className="rounded bg-accent px-4 py-2 text-sm text-white hover:bg-accent/90 disabled:opacity-50"
            >
              {loading ? "Creating\u2026" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
