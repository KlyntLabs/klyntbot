import { ipc } from "@shared/hooks/useIpc";
import type { ContextResumeData } from "@shared/types";
import { useCallback } from "react";
import { useNavigate } from "react-router";

export function useContextResume() {
  const navigate = useNavigate();

  const resume = useCallback(
    async (contextId: string) => {
      const data = await ipc<ContextResumeData>("get_context_resume_data", { contextId });
      navigate("/chat", {
        state: { resumeContext: data },
      });
      return data;
    },
    [navigate],
  );

  return { resume };
}
