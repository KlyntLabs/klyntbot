import type { Persona } from "../../hooks/usePersonas";

interface PersonaSelectorProps {
  personas: Persona[];
  selectedIds: string[];
  onSelect: (personaId: string) => void;
}

export function PersonaSelector({ personas, selectedIds, onSelect }: PersonaSelectorProps) {
  const available = personas.filter((p) => p.isActive && !selectedIds.includes(p.id));

  if (available.length === 0) return null;

  return (
    <select
      onChange={(e) => {
        if (e.target.value) onSelect(e.target.value);
        e.target.value = "";
      }}
      className="text-[10px] px-2 py-1 rounded-md bg-white/[0.04] text-muted border border-border"
      defaultValue=""
    >
      <option value="" disabled>
        Add persona...
      </option>
      {available.map((p) => (
        <option key={p.id} value={p.id}>
          {p.icon} {p.name} — {p.role}
        </option>
      ))}
    </select>
  );
}
