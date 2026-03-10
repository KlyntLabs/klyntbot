import { useCallback } from "react";
import { useLocation, useNavigate } from "react-router";
import { SETUP_STEPS } from "./steps";

export function useSetupNavigation() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  const currentIndex = SETUP_STEPS.findIndex((s) => s.path === pathname);
  const currentStep = SETUP_STEPS[currentIndex];
  const isFirst = currentIndex <= 0;
  const isLast = currentIndex >= SETUP_STEPS.length - 1;

  const next = useCallback(() => {
    if (!isLast) navigate(SETUP_STEPS[currentIndex + 1].path);
  }, [currentIndex, isLast, navigate]);

  const back = useCallback(() => {
    if (!isFirst) navigate(SETUP_STEPS[currentIndex - 1].path);
  }, [currentIndex, isFirst, navigate]);

  return {
    currentStep,
    currentIndex,
    totalSteps: SETUP_STEPS.length,
    isFirst,
    isLast,
    next,
    back,
    skip: next,
  };
}
