import { Plus, Settings2, Trash2, X } from "lucide-react";
import { useCallback, useState } from "react";
import type { Persona, PersonaActions } from "../../hooks/usePersonas";
import { useSquads } from "../../hooks/useSquads";

interface ManagePersonasModalProps {
  personas: Persona[];
  actions: PersonaActions;
  noteId: string | null;
  squadId?: string | null;
  onClose: () => void;
}

const TONE_OPTIONS = [
  "analytical",
  "curious",
  "pragmatic",
  "skeptical",
  "inquisitive",
  "provocative",
  "direct",
  "formal",
];

export function ManagePersonasModal({
  personas,
  actions,
  noteId,
  squadId,
  onClose,
}: ManagePersonasModalProps) {
  const [showCreate, setShowCreate] = useState(false);
  const [creating, setCreating] = useState(false);
  const [pinnedIds, setPinnedIds] = useState<Set<string> | null>(null);
  const [autoGenerating, setAutoGenerating] = useState(false);
  const { addMember } = useSquads();
  const [form, setForm] = useState({
    name: "",
    role: "",
    expertise: "",
    perspective: "",
    tone: "analytical",
    icon: "🧠",
    domains: "",
  });

  const handleCreate = useCallback(async () => {
    if (!form.name.trim() || !form.role.trim()) return;
    setCreating(true);
    try {
      const newPersona = await actions.create({
        name: form.name,
        role: form.role,
        expertise: form.expertise,
        perspective: form.perspective,
        tone: form.tone,
        icon: form.icon,
        domains: form.domains
          .split(",")
          .map((d) => d.trim().toLowerCase())
          .filter(Boolean),
      });
      if (squadId && newPersona) {
        await addMember({
          squadId,
          personaId: newPersona.id,
          roleInSquad: "member",
          sortOrder: 0,
        });
      }
      setShowCreate(false);
      setForm({
        name: "",
        role: "",
        expertise: "",
        perspective: "",
        tone: "analytical",
        icon: "🧠",
        domains: "",
      });
    } finally {
      setCreating(false);
    }
  }, [form, actions, squadId, addMember]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="glass-panel w-[480px] max-h-[80vh] rounded-xl flex flex-col">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-3 border-b border-border shrink-0">
          <Settings2 size={14} className="text-purple-400" />
          <span className="text-[13px] font-medium text-foreground flex-1">Manage Personas</span>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-white/[0.06]"
          >
            <X size={14} />
          </button>
        </div>

        {/* Persona list */}
        <div className="flex-1 overflow-y-auto p-3 space-y-2 min-h-0">
          {personas.map((p) => (
            <div
              key={p.id}
              className="flex items-center gap-2 p-2 rounded-lg bg-white/[0.03] group"
            >
              <span className="text-sm shrink-0">{p.icon}</span>
              <div className="flex-1 min-w-0">
                <div className="text-[11px] font-medium text-foreground truncate">{p.name}</div>
                <div className="text-2xs text-dim truncate">
                  {p.role} · {p.tone}
                  {p.source === "builtin" && (
                    <span className="ml-1 text-[9px] px-1 py-px rounded bg-white/[0.06]">
                      builtin
                    </span>
                  )}
                  {p.source === "auto" && (
                    <span className="ml-1 text-[9px] px-1 py-px rounded bg-purple-400/20 text-purple-300">
                      auto
                    </span>
                  )}
                </div>
              </div>
              <label className="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  checked={p.isActive}
                  onChange={(e) => actions.toggle(p.id, e.target.checked)}
                  className="size-3 accent-purple-400"
                />
                <span className="text-[9px] text-dim">Active</span>
              </label>
              {noteId && (
                <label className="flex items-center gap-1 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={pinnedIds?.has(p.id) ?? false}
                    onChange={(e) => {
                      // Lazily initialize from empty on first interaction
                      const current = pinnedIds ?? new Set<string>();
                      const next = new Set(current);
                      if (e.target.checked) next.add(p.id);
                      else next.delete(p.id);
                      setPinnedIds(next);
                      if (noteId) actions.setPins(noteId, [...next]);
                    }}
                    className="size-3 accent-amber-400"
                  />
                  <span className="text-[9px] text-dim">Pin</span>
                </label>
              )}
              {p.source !== "builtin" && (
                <button
                  type="button"
                  onClick={() => actions.remove(p.id)}
                  className="p-1 text-dim hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                  title="Delete persona"
                >
                  <Trash2 size={12} />
                </button>
              )}
            </div>
          ))}
        </div>

        {/* Create form */}
        {showCreate && (
          <div className="border-t border-border p-3 space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <input
                type="text"
                placeholder="Name"
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
              />
              <input
                type="text"
                placeholder="Role"
                value={form.role}
                onChange={(e) => setForm({ ...form, role: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
              />
            </div>
            <input
              type="text"
              placeholder="Expertise"
              value={form.expertise}
              onChange={(e) => setForm({ ...form, expertise: e.target.value })}
              className="w-full text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
            />
            <input
              type="text"
              placeholder="Perspective (how they analyze)"
              value={form.perspective}
              onChange={(e) => setForm({ ...form, perspective: e.target.value })}
              className="w-full text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
            />
            <div className="grid grid-cols-3 gap-2">
              <select
                value={form.tone}
                onChange={(e) => setForm({ ...form, tone: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
              >
                {TONE_OPTIONS.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
              <input
                type="text"
                placeholder="Icon emoji"
                value={form.icon}
                onChange={(e) => setForm({ ...form, icon: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
                maxLength={4}
              />
              <input
                type="text"
                placeholder="Domains (comma-sep)"
                value={form.domains}
                onChange={(e) => setForm({ ...form, domains: e.target.value })}
                className="text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border"
              />
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="text-2xs px-3 py-1 rounded-md text-muted-foreground hover:text-foreground"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleCreate}
                disabled={creating || !form.name.trim() || !form.role.trim()}
                className="text-2xs px-3 py-1 rounded-md bg-purple-400/20 text-purple-300 hover:bg-purple-400/30 disabled:opacity-50"
              >
                {creating ? "Creating..." : "Create"}
              </button>
            </div>
          </div>
        )}

        {/* Footer */}
        <div className="flex items-center gap-2 px-4 py-2.5 border-t border-border shrink-0">
          {!showCreate && (
            <button
              type="button"
              onClick={() => setShowCreate(true)}
              className="flex items-center gap-1 text-2xs px-2 py-1 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.06]"
            >
              <Plus size={10} />
              Create Persona
            </button>
          )}
          {noteId && !showCreate && (
            <button
              type="button"
              disabled={autoGenerating}
              onClick={async () => {
                setAutoGenerating(true);
                try {
                  await actions.autoGenerate(noteId);
                } finally {
                  setAutoGenerating(false);
                }
              }}
              className="flex items-center gap-1 text-2xs px-2 py-1 rounded-md bg-purple-400/10 text-purple-300 hover:bg-purple-400/20 disabled:opacity-50"
            >
              {autoGenerating ? "Generating..." : "Auto-generate"}
            </button>
          )}
          <div className="flex-1" />
          <button
            type="button"
            onClick={onClose}
            className="text-2xs px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
