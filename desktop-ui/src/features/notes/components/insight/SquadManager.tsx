import { ChevronRight, Crown, Plus, Settings2, Trash2, User, Users, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { usePersonas } from "../../hooks/usePersonas";
import type { Squad, SquadMember } from "../../hooks/useSquads";
import { useSquads } from "../../hooks/useSquads";

interface SquadManagerProps {
  open: boolean;
  onClose: () => void;
}

const SKILL_OPTIONS = [
  { value: "general", label: "General" },
  { value: "task-management", label: "Task Management" },
  { value: "finance-management", label: "Finance Management" },
  { value: "automation", label: "Automation" },
  { value: "communication", label: "Communication" },
];

const ROLE_OPTIONS = ["lead", "member"];

const memberLabel = (n: number) => `${n} ${n === 1 ? "member" : "members"}`;

const inputClass =
  "text-[11px] px-2 py-1.5 rounded-md bg-white/[0.04] text-foreground border border-border placeholder:text-dim";

export function SquadManager({ open, onClose }: SquadManagerProps) {
  const { squads, loading, createSquad, deleteSquad, addMember, removeMember } = useSquads();
  const [personas] = usePersonas();

  const [selectedSquadId, setSelectedSquadId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [creating, setCreating] = useState(false);
  const [addingPersonaId, setAddingPersonaId] = useState<string>("");
  const [addingRole, setAddingRole] = useState<string>("member");

  const [form, setForm] = useState({
    name: "",
    description: "",
    icon: "👥",
    orchestratorSkill: "general",
    domains: "",
    memberPersonaIds: [] as string[],
  });

  const selectedSquad = useMemo(
    () => squads.find((s) => s.id === selectedSquadId) ?? null,
    [squads, selectedSquadId],
  );

  // Auto-select first squad when list loads or selection becomes invalid
  useEffect(() => {
    if (squads.length > 0 && !squads.find((s) => s.id === selectedSquadId)) {
      setSelectedSquadId(squads[0].id);
    }
  }, [squads, selectedSquadId]);

  // Escape to close
  useEffect(() => {
    if (!open) return;
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open, onClose]);

  const handleCreate = useCallback(async () => {
    if (!form.name.trim()) return;
    setCreating(true);
    try {
      const squad = await createSquad({
        name: form.name,
        description: form.description,
        icon: form.icon,
        orchestratorSkill: form.orchestratorSkill,
        domains: form.domains
          .split(",")
          .map((d) => d.trim().toLowerCase())
          .filter(Boolean),
      });

      // Add selected members
      for (const personaId of form.memberPersonaIds) {
        await addMember({
          squadId: squad.id,
          personaId,
          roleInSquad: "member",
          sortOrder: 0,
        });
      }

      setSelectedSquadId(squad.id);
      setShowCreate(false);
      setForm({
        name: "",
        description: "",
        icon: "👥",
        orchestratorSkill: "general",
        domains: "",
        memberPersonaIds: [],
      });
    } finally {
      setCreating(false);
    }
  }, [form, createSquad, addMember]);

  const handleDeleteSquad = useCallback(
    async (id: string) => {
      await deleteSquad(id);
      if (selectedSquadId === id) {
        setSelectedSquadId(null);
      }
    },
    [deleteSquad, selectedSquadId],
  );

  const handleAddMember = useCallback(async () => {
    if (!selectedSquad || !addingPersonaId) return;
    await addMember({
      squadId: selectedSquad.id,
      personaId: addingPersonaId,
      roleInSquad: addingRole,
      sortOrder: selectedSquad.members.length,
    });
    setAddingPersonaId("");
    setAddingRole("member");
  }, [selectedSquad, addingPersonaId, addingRole, addMember]);

  const handleRemoveMember = useCallback(
    async (personaId: string) => {
      if (!selectedSquad) return;
      await removeMember(selectedSquad.id, personaId);
    },
    [selectedSquad, removeMember],
  );

  // Personas not already in the selected squad
  const availablePersonas = useMemo(() => {
    if (!selectedSquad) return personas;
    const memberIds = new Set(selectedSquad.members.map((m) => m.personaId));
    return personas.filter((p) => !memberIds.has(p.id));
  }, [personas, selectedSquad]);

  if (!open) return null;

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={onClose}
        role="presentation"
      />

      <div className="relative glass-panel w-[680px] max-h-[80vh] rounded-xl flex flex-col">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 py-3 border-b border-border shrink-0">
          <Users size={14} className="text-purple-400" />
          <span className="text-[13px] font-medium text-foreground flex-1">Manage Squads</span>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-white/[0.06]"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body: two columns */}
        <div className="flex flex-1 min-h-0">
          {/* Left column — Squad list */}
          <div className="w-[220px] border-r border-border flex flex-col shrink-0">
            <div className="flex-1 overflow-y-auto p-2 space-y-1">
              {loading && (
                <div className="px-2 py-4 text-[10px] text-dim text-center italic">Loading...</div>
              )}
              {!loading && squads.length === 0 && !showCreate && (
                <div className="px-2 py-4 text-[10px] text-dim text-center italic">
                  No squads yet
                </div>
              )}
              {squads.map((squad) => (
                <SquadListItem
                  key={squad.id}
                  squad={squad}
                  isSelected={squad.id === selectedSquadId}
                  onSelect={() => {
                    setSelectedSquadId(squad.id);
                    setShowCreate(false);
                  }}
                  onDelete={
                    squad.source !== "builtin" ? () => handleDeleteSquad(squad.id) : undefined
                  }
                />
              ))}
            </div>
            <div className="p-2 border-t border-border">
              <button
                type="button"
                onClick={() => {
                  setShowCreate(true);
                  setSelectedSquadId(null);
                }}
                className="flex items-center gap-1 w-full text-[10px] px-2 py-1.5 rounded-md bg-white/[0.04] text-muted-foreground hover:text-foreground hover:bg-white/[0.06] transition-colors"
              >
                <Plus size={10} />
                New Squad
              </button>
            </div>
          </div>

          {/* Right column — Detail / Create */}
          <div className="flex-1 overflow-y-auto p-4 min-w-0">
            {showCreate ? (
              <CreateSquadForm
                form={form}
                setForm={setForm}
                creating={creating}
                allPersonas={personas}
                selectedMemberIds={form.memberPersonaIds}
                onToggleMember={(id) => {
                  setForm((prev) => ({
                    ...prev,
                    memberPersonaIds: prev.memberPersonaIds.includes(id)
                      ? prev.memberPersonaIds.filter((x) => x !== id)
                      : [...prev.memberPersonaIds, id],
                  }));
                }}
                onCreate={handleCreate}
                onCancel={() => setShowCreate(false)}
              />
            ) : selectedSquad ? (
              <SquadDetail
                squad={selectedSquad}
                availablePersonas={availablePersonas}
                addingPersonaId={addingPersonaId}
                addingRole={addingRole}
                onSetAddingPersonaId={setAddingPersonaId}
                onSetAddingRole={setAddingRole}
                onAddMember={handleAddMember}
                onRemoveMember={handleRemoveMember}
              />
            ) : (
              <div className="flex items-center justify-center h-full text-[11px] text-dim italic">
                Select a squad or create a new one
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end px-4 py-2.5 border-t border-border shrink-0">
          <button
            type="button"
            onClick={onClose}
            className="text-[10px] px-3 py-1 rounded-md bg-white/[0.06] text-muted-foreground hover:text-foreground"
          >
            Done
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

// ── Squad list item ─────────────────────────────────────────────

function SquadListItem({
  squad,
  isSelected,
  onSelect,
  onDelete,
}: {
  squad: Squad;
  isSelected: boolean;
  onSelect: () => void;
  onDelete?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={`flex items-center gap-2 w-full px-2 py-1.5 rounded-md text-left group transition-colors ${
        isSelected
          ? "bg-purple/20 border border-purple/30"
          : "bg-transparent border border-transparent hover:bg-white/[0.04]"
      }`}
    >
      <span className="text-sm shrink-0">{squad.icon}</span>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1">
          <span className="text-[11px] font-medium text-foreground truncate">{squad.name}</span>
          {squad.source === "builtin" && (
            <span className="text-[8px] px-1 py-px rounded bg-white/[0.06] text-dim shrink-0">
              Built-in
            </span>
          )}
        </div>
        <div className="text-[9px] text-dim">{memberLabel(squad.members.length)}</div>
      </div>
      {isSelected && <ChevronRight size={10} className="text-dim shrink-0" />}
      {onDelete && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className="p-0.5 text-dim hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
          title="Delete squad"
        >
          <Trash2 size={10} />
        </button>
      )}
    </button>
  );
}

// ── Squad detail + member editor ─────────────────────────────────

function SquadDetail({
  squad,
  availablePersonas,
  addingPersonaId,
  addingRole,
  onSetAddingPersonaId,
  onSetAddingRole,
  onAddMember,
  onRemoveMember,
}: {
  squad: Squad;
  availablePersonas: { id: string; name: string; icon: string }[];
  addingPersonaId: string;
  addingRole: string;
  onSetAddingPersonaId: (id: string) => void;
  onSetAddingRole: (role: string) => void;
  onAddMember: () => void;
  onRemoveMember: (personaId: string) => void;
}) {
  const isUser = squad.source !== "builtin";

  return (
    <div className="space-y-4">
      {/* Squad header info */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <span className="text-lg">{squad.icon}</span>
          <h3 className="text-[13px] font-semibold text-foreground">{squad.name}</h3>
          {squad.source === "builtin" && (
            <span className="text-[8px] px-1.5 py-0.5 rounded bg-white/[0.06] text-dim">
              Built-in
            </span>
          )}
          {squad.source === "user" && (
            <span className="text-[8px] px-1.5 py-0.5 rounded bg-purple-400/20 text-purple-300">
              user
            </span>
          )}
        </div>
        {squad.description && (
          <p className="text-[11px] text-muted-foreground leading-relaxed">{squad.description}</p>
        )}
        <div className="flex items-center gap-3 mt-2">
          <span className="text-[10px] text-dim">
            <Settings2 size={9} className="inline mr-0.5" />
            {squad.orchestratorSkill}
          </span>
          {squad.domains.length > 0 && (
            <span className="text-[10px] text-dim">{squad.domains.join(", ")}</span>
          )}
        </div>
      </div>

      {/* Members list */}
      <div>
        <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">
          Members ({squad.members.length})
        </div>
        <div className="space-y-1">
          {squad.members.map((member) => (
            <MemberRow
              key={member.personaId}
              member={member}
              canRemove={isUser}
              onRemove={() => onRemoveMember(member.personaId)}
            />
          ))}
          {squad.members.length === 0 && (
            <div className="text-[10px] text-dim italic py-2">No members yet</div>
          )}
        </div>
      </div>

      {/* Add member (user squads only) */}
      {isUser && (
        <div>
          <div className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-2">
            Add Member
          </div>
          <div className="flex items-center gap-2">
            <select
              value={addingPersonaId}
              onChange={(e) => onSetAddingPersonaId(e.target.value)}
              className={`flex-1 ${inputClass}`}
            >
              <option value="">Select persona...</option>
              {availablePersonas.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.icon} {p.name}
                </option>
              ))}
            </select>
            <select
              value={addingRole}
              onChange={(e) => onSetAddingRole(e.target.value)}
              className={inputClass}
            >
              {ROLE_OPTIONS.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
            <button
              type="button"
              onClick={onAddMember}
              disabled={!addingPersonaId}
              className="text-[10px] px-2.5 py-1.5 rounded-md bg-purple-400/20 text-purple-300 hover:bg-purple-400/30 disabled:opacity-50 shrink-0"
            >
              Add
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Member row ──────────────────────────────────────────────────

function MemberRow({
  member,
  canRemove,
  onRemove,
}: {
  member: SquadMember;
  canRemove: boolean;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center gap-2 p-1.5 rounded-md bg-white/[0.03] group relative">
      <span className="text-sm shrink-0">{member.personaIcon}</span>
      <div className="flex-1 min-w-0">
        <div className="text-[11px] font-medium text-foreground truncate">{member.personaName}</div>
        <div className="text-[10px] text-dim truncate">{member.personaRole}</div>
      </div>
      <span className="flex items-center gap-0.5 text-[9px] text-dim shrink-0">
        {member.roleInSquad === "lead" ? (
          <Crown size={8} className="text-amber-400" />
        ) : (
          <User size={8} />
        )}
        {member.roleInSquad}
      </span>
      {canRemove && (
        <button
          type="button"
          onClick={onRemove}
          className="p-0.5 text-dim hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
          title="Remove member"
        >
          <Trash2 size={10} />
        </button>
      )}

      {/* Tooltip on hover (CSS-driven) */}
      <div className="absolute left-full ml-2 top-0 z-10 glass-panel rounded-lg p-2 w-48 shadow-xl pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity">
        <div className="text-[11px] font-medium text-foreground">{member.personaName}</div>
        <div className="text-[10px] text-muted-foreground mt-0.5">{member.personaRole}</div>
        <div className="text-[9px] text-dim mt-1">
          Role in squad: <span className="text-muted-foreground">{member.roleInSquad}</span>
        </div>
      </div>
    </div>
  );
}

// ── Create squad form ───────────────────────────────────────────

function CreateSquadForm({
  form,
  setForm,
  creating,
  allPersonas,
  selectedMemberIds,
  onToggleMember,
  onCreate,
  onCancel,
}: {
  form: {
    name: string;
    description: string;
    icon: string;
    orchestratorSkill: string;
    domains: string;
    memberPersonaIds: string[];
  };
  setForm: React.Dispatch<
    React.SetStateAction<{
      name: string;
      description: string;
      icon: string;
      orchestratorSkill: string;
      domains: string;
      memberPersonaIds: string[];
    }>
  >;
  creating: boolean;
  allPersonas: { id: string; name: string; icon: string }[];
  selectedMemberIds: string[];
  onToggleMember: (id: string) => void;
  onCreate: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="space-y-3">
      <div className="text-[12px] font-medium text-foreground">Create New Squad</div>

      <div className="grid grid-cols-[1fr_60px] gap-2">
        <input
          type="text"
          placeholder="Squad name"
          value={form.name}
          onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
          className={inputClass}
        />
        <input
          type="text"
          placeholder="Icon"
          value={form.icon}
          onChange={(e) => setForm((prev) => ({ ...prev, icon: e.target.value }))}
          className={`${inputClass} text-center`}
          maxLength={4}
        />
      </div>

      <textarea
        placeholder="Description"
        value={form.description}
        onChange={(e) => setForm((prev) => ({ ...prev, description: e.target.value }))}
        className={`w-full ${inputClass} resize-none`}
        rows={2}
      />

      <div className="grid grid-cols-2 gap-2">
        <label className="block">
          <span className="text-[10px] text-dim block mb-1">Orchestrator Skill</span>
          <select
            value={form.orchestratorSkill}
            onChange={(e) =>
              setForm((prev) => ({
                ...prev,
                orchestratorSkill: e.target.value,
              }))
            }
            className={`w-full ${inputClass}`}
          >
            {SKILL_OPTIONS.map((s) => (
              <option key={s.value} value={s.value}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          <span className="text-[10px] text-dim block mb-1">Domains (comma-sep)</span>
          <input
            type="text"
            placeholder="e.g. code, devops"
            value={form.domains}
            onChange={(e) => setForm((prev) => ({ ...prev, domains: e.target.value }))}
            className={`w-full ${inputClass}`}
          />
        </label>
      </div>

      {/* Persona multi-select */}
      <div>
        <span className="text-[10px] text-dim block mb-1">
          Members ({selectedMemberIds.length} selected)
        </span>
        <div className="glass-card rounded-md p-2 max-h-[140px] overflow-y-auto space-y-1">
          {allPersonas.map((p) => {
            const isSelected = selectedMemberIds.includes(p.id);
            return (
              <label
                key={p.id}
                className="flex items-center gap-2 px-1.5 py-1 rounded-md hover:bg-white/[0.04] cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => onToggleMember(p.id)}
                  className="w-3 h-3 accent-purple-400"
                />
                <span className="text-sm">{p.icon}</span>
                <span className="text-[11px] text-foreground truncate">{p.name}</span>
              </label>
            );
          })}
          {allPersonas.length === 0 && (
            <div className="text-[10px] text-dim italic py-1">No personas available</div>
          )}
        </div>
      </div>

      <div className="flex justify-end gap-2 pt-1">
        <button
          type="button"
          onClick={onCancel}
          className="text-[10px] px-3 py-1 rounded-md text-muted-foreground hover:text-foreground"
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={onCreate}
          disabled={creating || !form.name.trim()}
          className="text-[10px] px-3 py-1.5 rounded-md bg-purple-400/20 text-purple-300 hover:bg-purple-400/30 disabled:opacity-50"
        >
          {creating ? "Creating..." : "Create Squad"}
        </button>
      </div>
    </div>
  );
}
