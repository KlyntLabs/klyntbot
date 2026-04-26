import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useEffect, useState } from "react";

export interface Persona {
  id: string;
  name: string;
  role: string;
  expertise: string;
  perspective: string;
  tone: string;
  icon: string;
  source: string;
  domains: string[];
  isActive: boolean;
  relevanceScore: number;
  createdAt: string;
  updatedAt: string;
}

interface CreatePersonaInput {
  name: string;
  role: string;
  expertise: string;
  perspective: string;
  tone: string;
  icon: string;
  domains: string[];
}

interface UpdatePersonaInput {
  id: string;
  name?: string;
  role?: string;
  expertise?: string;
  perspective?: string;
  tone?: string;
  icon?: string;
  domains?: string[];
}

export interface PersonaActions {
  refresh: () => Promise<void>;
  create: (input: CreatePersonaInput) => Promise<Persona>;
  update: (input: UpdatePersonaInput) => Promise<Persona>;
  remove: (id: string) => Promise<void>;
  toggle: (id: string, active: boolean) => Promise<void>;
  setPins: (noteId: string, personaIds: string[]) => Promise<void>;
  rate: (id: string, helpful: boolean) => Promise<void>;
  autoGenerate: (noteId: string) => Promise<Persona>;
}

export function usePersonas(): [Persona[], PersonaActions] {
  const [personas, setPersonas] = useState<Persona[]>([]);

  const refresh = useCallback(async () => {
    const result = await ipc<Persona[]>("note_insight_list_personas", {});
    setPersonas(result);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const create = useCallback(
    async (input: CreatePersonaInput) => {
      const result = await ipc<Persona>("note_insight_create_persona", { params: input });
      await refresh();
      return result;
    },
    [refresh],
  );

  const update = useCallback(
    async (input: UpdatePersonaInput) => {
      const result = await ipc<Persona>("note_insight_update_persona", { params: input });
      await refresh();
      return result;
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await ipc("note_insight_delete_persona", { id });
      await refresh();
    },
    [refresh],
  );

  const toggle = useCallback(
    async (id: string, active: boolean) => {
      await ipc("note_insight_toggle_persona", { id, active });
      await refresh();
    },
    [refresh],
  );

  const setPins = useCallback(async (noteId: string, personaIds: string[]) => {
    await ipc("note_insight_set_pins", { params: { noteId, personaIds } });
  }, []);

  const rate = useCallback(async (id: string, helpful: boolean) => {
    await ipc("note_insight_rate_persona", { params: { id, helpful } });
  }, []);

  const autoGenerate = useCallback(
    async (noteId: string) => {
      const result = await ipc<Persona>("note_insight_auto_generate_persona", { noteId });
      await refresh();
      return result;
    },
    [refresh],
  );

  return [personas, { refresh, create, update, remove, toggle, setPins, rate, autoGenerate }];
}
