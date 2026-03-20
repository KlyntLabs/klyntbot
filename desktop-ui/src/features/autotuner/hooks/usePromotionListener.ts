import { useEffect, useRef } from "react";
import { useAutoTunerStatus } from "./useAutoTunerStatus";

const MAX_PROMOTIONS = 3;

export function usePromotionListener(onPromotion: (impact: string) => void) {
  const { data: status } = useAutoTunerStatus();
  const prevTrialIdRef = useRef<string | null | undefined>(undefined);
  const callbackRef = useRef(onPromotion);
  const countRef = useRef(0);

  callbackRef.current = onPromotion;

  useEffect(() => {
    const currentTrialId = status?.champion?.trial_id ?? null;

    // First render — just record the current value, don't fire
    if (prevTrialIdRef.current === undefined) {
      prevTrialIdRef.current = currentTrialId;
      return;
    }

    // Detect a promotion: trial_id changed to a new non-null value
    if (
      currentTrialId !== null &&
      currentTrialId !== prevTrialIdRef.current &&
      countRef.current < MAX_PROMOTIONS
    ) {
      countRef.current += 1;
      callbackRef.current(status?.champion?.impact ?? "Tuning applied");
    }

    prevTrialIdRef.current = currentTrialId;
  }, [status?.champion?.trial_id, status?.champion?.impact]);
}
