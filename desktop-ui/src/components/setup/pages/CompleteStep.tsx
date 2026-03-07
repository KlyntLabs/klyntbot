import { CheckCircle } from "lucide-react";
import { useNavigate } from "react-router";
import { ipc } from "../../../hooks/useIpc";

export function CompleteStep() {
  const navigate = useNavigate();

  const handleLaunch = async () => {
    await ipc("config_mark_setup_completed").catch(() => {});
    navigate("/");
  };

  return (
    <div className="text-center py-4">
      <CheckCircle className="w-12 h-12 text-success mx-auto mb-4" strokeWidth={1.5} />
      <h2 className="text-lg font-medium text-primary mb-2">You're all set!</h2>
      <p className="text-[13px] text-muted mb-8">
        Klynt is configured and ready to go. You can always change these settings later.
      </p>
      <button
        type="button"
        onClick={handleLaunch}
        className="px-6 py-2.5 text-[13px] font-medium text-white bg-brand hover:bg-brand-hover rounded-xl transition-colors"
      >
        Launch Klynt
      </button>
    </div>
  );
}
