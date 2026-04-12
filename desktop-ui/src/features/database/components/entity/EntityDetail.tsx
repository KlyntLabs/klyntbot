import { useDeleteEntity, useUpdateEntity } from "@features/database/hooks/useEntity";
import { getEntityTitle } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity } from "@shared/types";
import { useState } from "react";
import { PropertyList } from "./PropertyList";

interface EntityDetailProps {
  schema: DatabaseSchema;
  entity: Entity;
  onClose: () => void;
}

export function EntityDetail({ schema, entity, onClose }: EntityDetailProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Record<string, unknown>>({ ...entity.fields });
  const { mutate: updateEntity, loading: saving } = useUpdateEntity(schema.id);
  const { mutate: deleteEntity } = useDeleteEntity(schema.id);

  const handleChange = (slug: string, value: unknown) => {
    setDraft((prev) => ({ ...prev, [slug]: value }));
  };

  const handleSave = async () => {
    await updateEntity(entity.id, { fields: draft });
    setEditing(false);
  };

  const handleDelete = async () => {
    await deleteEntity(entity.id);
    onClose();
  };

  const title = getEntityTitle(schema, entity.fields);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 className="text-lg font-semibold truncate">{title}</h2>
        <div className="flex items-center gap-2">
          {editing ? (
            <>
              <button
                type="button"
                onClick={() => {
                  setEditing(false);
                  setDraft({ ...entity.fields });
                }}
                className="rounded px-3 py-1 text-sm text-muted hover:bg-surface-hover"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleSave}
                disabled={saving}
                className="rounded bg-accent px-3 py-1 text-sm text-white hover:bg-accent/90 disabled:opacity-50"
              >
                {saving ? "Saving\u2026" : "Save"}
              </button>
            </>
          ) : (
            <>
              <button
                type="button"
                onClick={() => setEditing(true)}
                className="rounded px-3 py-1 text-sm text-muted hover:bg-surface-hover"
              >
                Edit
              </button>
              <button
                type="button"
                onClick={handleDelete}
                className="rounded px-3 py-1 text-sm text-red-400 hover:bg-red-500/10"
              >
                Delete
              </button>
            </>
          )}
          <button
            type="button"
            onClick={onClose}
            className="rounded p-1 text-muted hover:bg-surface-hover"
          >
            {"\u2715"}
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        <PropertyList
          schema={schema}
          entity={editing ? { ...entity, fields: draft } : entity}
          editing={editing}
          onChange={handleChange}
        />
      </div>
      <div className="border-t border-border px-4 py-2 text-xs text-muted">
        Created {new Date(entity.createdAt).toLocaleString()} · Updated{" "}
        {new Date(entity.updatedAt).toLocaleString()}
      </div>
    </div>
  );
}
