import { useMutation } from "@shared/hooks/useMutation";
import type { Project } from "@shared/types";
import { X } from "lucide-react";
import { useCallback, useState } from "react";

interface RolePanelProps {
  project: Project;
  onClose: () => void;
}

export function RolePanel({ project, onClose }: RolePanelProps) {
  const [role, setRole] = useState(project.userRole ?? "");
  const [dirty, setDirty] = useState(false);

  const updateRole = useMutation<boolean, { id: string; role: string }>("project_update_role");

  const handleSave = useCallback(async () => {
    await updateRole.mutate({ id: project.id, role: role.trim() });
    setDirty(false);
  }, [project.id, role, updateRole]);

  return (
    <div className="w-72 border-r border-white/[0.06] flex flex-col overflow-hidden shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/[0.06]">
        <h3 className="text-[13px] font-medium text-primary">My Role</h3>
        <button
          type="button"
          onClick={onClose}
          className="text-muted hover:text-secondary transition-colors"
        >
          <X className="w-4 h-4" strokeWidth={1.5} />
        </button>
      </div>

      <div className="flex-1 p-4">
        <p className="text-[11px] text-dim font-light mb-3">
          Describe your role in this project. The AI will tailor its suggestions based on your
          responsibilities.
        </p>
        <textarea
          value={role}
          onChange={(e) => {
            setRole(e.target.value);
            setDirty(true);
          }}
          placeholder="e.g. Tech Lead, Product Manager, Designer..."
          rows={4}
          className="w-full bg-white/[0.04] rounded-md px-3 py-2 text-[12px] font-light text-primary placeholder:text-dim resize-none outline-none border border-white/[0.06] focus:border-brand/40 transition-colors"
        />
      </div>

      {dirty && (
        <div className="px-4 py-3 border-t border-white/[0.06]">
          <button
            type="button"
            onClick={handleSave}
            className="w-full px-3 py-2 rounded-md bg-brand hover:bg-brand-hover text-white text-[12px] font-medium transition-colors"
          >
            Save Role
          </button>
        </div>
      )}
    </div>
  );
}
