import { Save } from "lucide-react";

interface SaveButtonProps {
  onClick: () => void;
  saving: boolean;
  disabled?: boolean;
}

export function SaveButton({ onClick, saving, disabled }: SaveButtonProps) {
  return (
    <div className="flex justify-end">
      <button
        type="button"
        onClick={onClick}
        disabled={saving || disabled}
        className="flex items-center gap-1.5 px-3 py-1.5 text-[11px] font-medium text-white bg-brand hover:bg-brand-hover rounded-lg transition-colors disabled:opacity-50"
      >
        <Save className="w-3 h-3" />
        {saving ? "Saving..." : "Save"}
      </button>
    </div>
  );
}
