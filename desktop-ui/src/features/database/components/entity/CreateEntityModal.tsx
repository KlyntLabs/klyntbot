import { FieldEditor } from "@features/database/components/fields/FieldEditor";
import { useCreateEntity } from "@features/database/hooks/useEntity";
import type { DatabaseSchema } from "@shared/types";
import { Button } from "@shared/ui/Button";
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
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div
        role="dialog"
        className="w-full max-w-md rounded-xl border border-border bg-card shadow-2xl"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-3.5">
          <h2 className="text-base font-semibold text-foreground">New {schema.name}</h2>
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
        <form onSubmit={handleSubmit} className="p-5 space-y-4">
          {editableFields.map((field) => (
            <div key={field.id} className="space-y-1.5">
              <label className="block text-[13px] font-medium text-foreground">
                {field.name}
                {field.required && <span className="text-destructive ml-0.5">*</span>}
              </label>
              <FieldEditor
                field={field}
                value={fields[field.slug]}
                onChange={(v) => setFields((prev) => ({ ...prev, [field.slug]: v }))}
              />
            </div>
          ))}
          {error && <p className="text-sm text-destructive">{error.message}</p>}
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="ghost" size="md" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" size="md" loading={loading}>
              Create
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
