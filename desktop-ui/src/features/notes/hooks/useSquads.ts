import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useEffect, useState } from "react";

// ── Types ────────────────────────────────────────────────────────

export interface SquadMember {
  personaId: string;
  personaName: string;
  personaIcon: string;
  personaRole: string;
  roleInSquad: string;
  sortOrder: number;
}

export interface Squad {
  id: string;
  name: string;
  description: string;
  icon: string;
  orchestratorSkill: string;
  source: string;
  domains: string[];
  isActive: boolean;
  members: SquadMember[];
  createdAt: string;
  updatedAt: string;
}

interface CreateSquadInput {
  name: string;
  description: string;
  icon: string;
  orchestratorSkill: string;
  domains: string[];
}

interface AddMemberInput {
  squadId: string;
  personaId: string;
  roleInSquad: string;
  sortOrder: number;
}

// ── Hook ─────────────────────────────────────────────────────────

export interface SquadActions {
  refresh: () => Promise<void>;
  createSquad: (input: CreateSquadInput) => Promise<Squad>;
  deleteSquad: (id: string) => Promise<void>;
  addMember: (input: AddMemberInput) => Promise<void>;
  removeMember: (squadId: string, personaId: string) => Promise<void>;
}

export function useSquads(): {
  squads: Squad[];
  loading: boolean;
} & SquadActions {
  const [squads, setSquads] = useState<Squad[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const result = await ipc<Squad[]>("list_squads", {});
      setSquads(result);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const createSquad = useCallback(
    async (input: CreateSquadInput) => {
      const result = await ipc<Squad>("create_squad", { params: input });
      await refresh();
      return result;
    },
    [refresh],
  );

  const deleteSquad = useCallback(
    async (id: string) => {
      await ipc("delete_squad", { id });
      await refresh();
    },
    [refresh],
  );

  const addMember = useCallback(
    async (input: AddMemberInput) => {
      await ipc("add_squad_member", { params: input });
      await refresh();
    },
    [refresh],
  );

  const removeMember = useCallback(
    async (squadId: string, personaId: string) => {
      await ipc("remove_squad_member", { squadId, personaId });
      await refresh();
    },
    [refresh],
  );

  return { squads, loading, refresh, createSquad, deleteSquad, addMember, removeMember };
}
