import { useMutation } from "@shared/hooks/useMutation";
import type { KeyResultCreateParams } from "@shared/types";
import { useCallback, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";

interface KeyResultCreateFormProps {
  objectiveId: string;
  onCreated: () => void;
  onCancel: () => void;
}

export function KeyResultCreateForm({
  objectiveId,
  onCreated,
  onCancel,
}: KeyResultCreateFormProps) {
  const { refetchObjectives } = useProjectContext();
  const [title, setTitle] = useState("");
  const [targetValue, setTargetValue] = useState("");
  const [unit, setUnit] = useState("");
  const [trackingMode, setTrackingMode] = useState("manual");

  const { mutate, loading } = useMutation<void, KeyResultCreateParams>(
    "key_result_create",
    "params",
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!title.trim()) return;

      const params: KeyResultCreateParams = {
        objectiveId,
        title: title.trim(),
      };
      if (targetValue) params.targetValue = Number.parseFloat(targetValue);
      if (unit.trim()) params.unit = unit.trim();
      if (trackingMode !== "manual") params.trackingMode = trackingMode;

      await mutate(params);
      refetchObjectives();
      onCreated();
    },
    [title, targetValue, unit, trackingMode, objectiveId, mutate, refetchObjectives, onCreated],
  );

  return (
    <form
      onSubmit={handleSubmit}
      className="ml-4 p-3 rounded-lg border border-dashed border-separator bg-control-hover/30 space-y-2"
    >
      <input
        type="text"
        placeholder="Key Result title..."
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        className="w-full px-2.5 py-1.5 text-ui-sm bg-transparent border border-separator rounded text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
      />

      <div className="flex items-center gap-2">
        <input
          type="number"
          placeholder="Target"
          value={targetValue}
          onChange={(e) => setTargetValue(e.target.value)}
          className="w-20 px-2 py-1.5 text-ui-sm bg-transparent border border-separator rounded text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
        />
        <input
          type="text"
          placeholder="Unit (e.g. %)"
          value={unit}
          onChange={(e) => setUnit(e.target.value)}
          className="w-24 px-2 py-1.5 text-ui-sm bg-transparent border border-separator rounded text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
        />
        <select
          value={trackingMode}
          onChange={(e) => setTrackingMode(e.target.value)}
          className="px-2 py-1.5 text-ui-sm bg-transparent border border-separator rounded text-fg focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
        >
          <option value="manual">Manual</option>
          <option value="task_count">Task Count</option>
          <option value="percentage">Percentage</option>
        </select>
      </div>

      <div className="flex items-center gap-2 pt-1">
        <button
          type="submit"
          disabled={!title.trim() || loading}
          className="px-3 py-1.5 text-ui-xs font-medium rounded bg-brand text-white hover:bg-brand/90 disabled:opacity-50 transition-colors"
        >
          {loading ? "Creating..." : "Add KR"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-3 py-1.5 text-ui-xs text-fg-secondary hover:text-fg transition-colors"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
