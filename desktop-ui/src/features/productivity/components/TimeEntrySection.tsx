import { Clock, Plus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration } from "@shared/lib/dates";
import type { ActivityCategory, TimeEntry } from "@shared/types";

interface CreateEntryParams {
  description: string;
  duration_mins: number;
  category_id?: string;
}

interface DeleteEntryParams {
  id: number;
}

interface TimeEntrySectionProps {
  date: string;
}

export function TimeEntrySection({ date }: TimeEntrySectionProps) {
  const { data: entries, refetch } = useQuery<TimeEntry[]>(
    "productivity_time_entries",
    { date },
    [],
  );
  const { data: categories } = useQuery<ActivityCategory[]>(
    "productivity_categories",
    undefined,
    [],
  );
  const { mutate: createEntry } = useMutation<void, CreateEntryParams>(
    "productivity_time_entry_create",
  );
  const { mutate: deleteEntry } = useMutation<void, DeleteEntryParams>(
    "productivity_time_entry_delete",
  );

  const [showForm, setShowForm] = useState(false);
  const [description, setDescription] = useState("");
  const [durationMins, setDurationMins] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const formRef = useRef<HTMLDivElement>(null);

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload?.entityKind === "productivity") refetch();
  });

  useEffect(() => {
    if (!showForm) return;
    const handler = (e: MouseEvent) => {
      if (formRef.current && !formRef.current.contains(e.target as Node)) {
        setShowForm(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showForm]);

  const handleAdd = async () => {
    if (!description.trim() || !durationMins) return;
    await createEntry({
      description: description.trim(),
      duration_mins: Number(durationMins),
      category_id: categoryId || undefined,
    });
    setDescription("");
    setDurationMins("");
    setCategoryId("");
    setShowForm(false);
    refetch();
  };

  const handleDelete = async (id: number) => {
    await deleteEntry({ id });
    refetch();
  };

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-[13px] font-medium text-secondary flex items-center gap-2">
          <Clock className="w-3.5 h-3.5 text-muted" />
          Time Entries
        </h2>
        <button
          type="button"
          onClick={() => setShowForm(!showForm)}
          className="w-6 h-6 rounded-md flex items-center justify-center text-muted hover:text-brand hover:bg-white/[0.08] transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      {showForm && (
        <div
          ref={formRef}
          className="flex flex-col gap-2 p-3 bg-white/[0.02] rounded-lg border border-white/[0.08]"
        >
          <input
            type="text"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What did you work on?"
            className="w-full px-3 py-1.5 text-[13px] bg-white/[0.06] border border-white/[0.08] rounded-md text-primary placeholder:text-dim"
          />
          <div className="flex gap-2">
            <input
              type="number"
              value={durationMins}
              onChange={(e) => setDurationMins(e.target.value)}
              placeholder="Minutes"
              min={1}
              className="w-24 px-3 py-1.5 text-[13px] bg-white/[0.06] border border-white/[0.08] rounded-md text-primary placeholder:text-dim"
            />
            <select
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              className="flex-1 px-3 py-1.5 text-[13px] bg-white/[0.06] border border-white/[0.08] rounded-md text-primary"
            >
              <option value="">No category</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={handleAdd}
              disabled={!description.trim() || !durationMins}
              className="px-3 py-1.5 text-[12px] rounded-md bg-brand text-white hover:bg-brand-hover transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Log
            </button>
          </div>
        </div>
      )}

      {entries.length === 0 && !showForm ? (
        <p className="text-[12px] font-light text-dim">No manual entries today</p>
      ) : (
        <div className="flex flex-col gap-1.5">
          {entries.map((e) => (
            <div
              key={e.id}
              className="group flex items-center justify-between py-1.5 text-[12px] font-light"
            >
              <div className="flex items-center gap-2">
                <span className="text-primary">{e.description}</span>
                {e.categoryId && (
                  <span className="text-dim">
                    {categories.find((c) => c.id === e.categoryId)?.name}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <span className="text-muted tabular-nums">
                  {formatHumanDuration(e.durationSecs)}
                </span>
                <button
                  type="button"
                  onClick={() => handleDelete(e.id)}
                  className="w-5 h-5 rounded flex items-center justify-center text-transparent group-hover:text-muted hover:!text-destructive transition-colors"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
