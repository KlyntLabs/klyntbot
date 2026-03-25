import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { useEffect, useRef } from "react";
import { useAutoTunerStatus } from "./useAutoTunerStatus";

const MAX_PROMOTIONS = 3;

export function usePromotionListener(onPromotion: (impact: string) => void) {
  const { data: status } = useAutoTunerStatus();
  const { data: toastCount } = useQuery<number>("autotuner_get_toast_count", undefined, 0);
  const { mutate: incrementToastCount } = useMutation<number>("autotuner_increment_toast_count");
  const prevTrialIdRef = useRef<string | null | undefined>(undefined);
  const callbackRef = useRef(onPromotion);
  const toastCountRef = useRef(toastCount ?? 0);

  callbackRef.current = onPromotion;

  useEffect(() => {
    toastCountRef.current = toastCount ?? 0;
  }, [toastCount]);

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
      toastCountRef.current < MAX_PROMOTIONS
    ) {
      incrementToastCount({});
      callbackRef.current(status?.champion?.impact ?? "Tuning applied");
    }

    prevTrialIdRef.current = currentTrialId;
  }, [status?.champion?.trial_id, status?.champion?.impact, incrementToastCount]);
}
