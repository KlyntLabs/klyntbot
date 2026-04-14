import { PointerSensor, useSensor, useSensors } from "@dnd-kit/core";
import { useReducedMotion } from "@shared/hooks/useReducedMotion";

const DEFAULT_DISTANCE = 5;
const REDUCED_DISTANCE = 12;

export function useEntityDndSensors() {
  const reduced = useReducedMotion();
  return useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: reduced ? REDUCED_DISTANCE : DEFAULT_DISTANCE },
    }),
  );
}
