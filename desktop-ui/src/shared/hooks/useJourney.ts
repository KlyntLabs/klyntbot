import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";

export type MilestoneName =
  | "setup_complete"
  | "first_import"
  | "first_chat_response"
  | "orb_awakening"
  | "first_focus_debrief"
  | "first_dot_accepted"
  | "quiet_day"
  | "first_brain_report"
  | "hello_pulse";

export function useJourney() {
  const { data: milestones, refetch } = useQuery<string[]>("journey_milestones", undefined, []);

  const isComplete = (milestone: MilestoneName): boolean =>
    milestones?.includes(milestone) ?? false;

  const markComplete = async (milestone: MilestoneName) => {
    await ipc("journey_mark_complete", { milestone });
    refetch();
  };

  return { isComplete, markComplete, milestones: milestones ?? [] };
}
