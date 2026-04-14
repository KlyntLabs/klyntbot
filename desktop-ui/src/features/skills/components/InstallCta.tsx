import { ipc } from "@shared/hooks/useIpc";
import type { InstallPlan } from "@shared/types";
import { useState } from "react";
import { InstallPreviewDialog } from "./InstallPreviewDialog";

interface Props {
  sourceRef: string;
}

export function InstallCta({ sourceRef }: Props) {
  const [plan, setPlan] = useState<InstallPlan | null>(null);
  const [loading, setLoading] = useState(false);

  const openPreview = async () => {
    setLoading(true);
    try {
      const p = await ipc<InstallPlan>("skill_install_preview", { shorthand: sourceRef });
      setPlan(p);
    } finally {
      setLoading(false);
    }
  };

  return (
    <>
      <button
        type="button"
        disabled={loading}
        onClick={openPreview}
        className="px-4 py-2 bg-brand text-white rounded text-sm disabled:opacity-50"
      >
        {loading ? "Fetching..." : "Install"}
      </button>
      {plan && <InstallPreviewDialog plan={plan} onClose={() => setPlan(null)} />}
    </>
  );
}
