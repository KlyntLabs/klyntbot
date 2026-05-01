import { useCallback, useMemo } from "react";
import { transformAgentRouted } from "../slash/agentRouted";
import { classify } from "../slash/classify";
import { dispatchDirect } from "../slash/direct";
import { FLAT_CATALOG } from "../slash/registry";
import type { DispatchResult } from "../slash/types";

export function useSlashCommands() {
  const catalog = useCallback(() => FLAT_CATALOG, []);

  const dispatch = useCallback(
    async (input: string, sessionKey: string): Promise<DispatchResult> => {
      const verdict = classify(input);
      if (verdict === "agent") return transformAgentRouted(input);
      if (verdict === "direct") return dispatchDirect(input, sessionKey);
      return { kind: "passthrough", text: input };
    },
    [],
  );

  return useMemo(() => ({ catalog, classify, dispatch }), [catalog, dispatch]);
}
