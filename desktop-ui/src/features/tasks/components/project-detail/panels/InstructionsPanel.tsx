import { useMutation } from "@shared/hooks/useMutation";
import type { Project } from "@shared/types";
import { X } from "lucide-react";
import { useCallback, useState } from "react";

interface InstructionsPanelProps {
  project: Project;
  onClose: () => void;
}

const SECTIONS = [
  { key: "context", label: "Context", placeholder: "What is this project about?" },
  { key: "guidelines", label: "Guidelines", placeholder: "How should the AI approach this?" },
  { key: "constraints", label: "Constraints", placeholder: "What should the AI avoid?" },
  { key: "persona", label: "Persona", placeholder: "What personality should the AI have?" },
] as const;

type SectionKey = (typeof SECTIONS)[number]["key"];

export function InstructionsPanel({ project, onClose }: InstructionsPanelProps) {
  const [values, setValues] = useState<Record<string, string>>(() => {
    const inst = project.instructions ?? {};
    return {
      context: inst.context ?? "",
      guidelines: inst.guidelines ?? "",
      constraints: inst.constraints ?? "",
      persona: inst.persona ?? "",
    };
  });
  const [dirty, setDirty] = useState(false);

  const updateInstructions = useMutation<boolean, { id: string; instructions: string }>(
    "project_update_instructions",
  );

  const handleChange = (key: SectionKey, value: string) => {
    setValues((prev) => ({ ...prev, [key]: value }));
    setDirty(true);
  };

  const handleSave = useCallback(async () => {
    const instructions: Record<string, string> = {};
    for (const s of SECTIONS) {
      if (values[s.key]?.trim()) {
        instructions[s.key] = values[s.key].trim();
      }
    }
    await updateInstructions.mutate({
      id: project.id,
      instructions: JSON.stringify(instructions),
    });
    setDirty(false);
  }, [project.id, values, updateInstructions]);

  return (
    <div className="w-80 border-r border-white/[0.06] flex flex-col overflow-hidden shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/[0.06]">
        <h3 className="text-[13px] font-medium text-primary">Instructions</h3>
        <button
          type="button"
          onClick={onClose}
          className="text-muted hover:text-secondary transition-colors"
        >
          <X className="w-4 h-4" strokeWidth={1.5} />
        </button>
      </div>

      {/* Sections */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {SECTIONS.map((section) => (
          <div key={section.key}>
            <label className="block text-[11px] font-medium text-dim uppercase tracking-wider mb-1.5">
              {section.label}
            </label>
            <textarea
              value={values[section.key] ?? ""}
              onChange={(e) => handleChange(section.key, e.target.value)}
              placeholder={section.placeholder}
              rows={3}
              className="w-full bg-white/[0.04] rounded-md px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim resize-none outline-none border border-white/[0.06] focus:border-brand/40 transition-colors"
            />
          </div>
        ))}
      </div>

      {/* Save */}
      {dirty && (
        <div className="px-4 py-3 border-t border-white/[0.06]">
          <button
            type="button"
            onClick={handleSave}
            className="w-full px-3 py-2 rounded-md bg-brand hover:bg-brand-hover text-white text-[12px] font-medium transition-colors"
          >
            Save Instructions
          </button>
        </div>
      )}
    </div>
  );
}
