import { useCallback, useState } from "react";
import { invoke } from "@/api/client";

interface Props {
  requestId: string;
  initialDraft?: string;
  onCommit: (path: string) => void;
  onCancel: () => void;
}

export function StarlarkRuleEditor({ requestId, initialDraft, onCommit, onCancel }: Props) {
  const [src, setSrc] = useState(
    initialDraft ?? `prefix_rule(["git", "status"], decision="allow")\n`,
  );
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const onSave = useCallback(async () => {
    setSaving(true);
    setError(null);
    try {
      const path = await invoke<string>("chat_save_starlark_rule", {
        requestId,
        ruleSource: src,
        suggestedFilename: null,
      });
      onCommit(path);
    } catch (e: any) {
      setError(String(e?.message ?? e));
    } finally {
      setSaving(false);
    }
  }, [requestId, src, onCommit]);

  return (
    <div className="starlark-rule-editor">
      <h4>Add Starlark rule</h4>
      <textarea
        value={src}
        onChange={(e) => setSrc(e.target.value)}
        rows={8}
        spellCheck={false}
        autoFocus
      />
      {error && <div className="starlark-rule-editor__error">{error}</div>}
      <div className="starlark-rule-editor__actions">
        <button onClick={onCancel} disabled={saving}>
          Cancel
        </button>
        <button onClick={onSave} disabled={saving} className="starlark-rule-editor__primary">
          {saving ? "Saving..." : "Save rule"}
        </button>
      </div>
    </div>
  );
}
