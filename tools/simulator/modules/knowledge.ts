// tools/simulator/modules/knowledge.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";

interface AtomResponse { id: string; status: string; [key: string]: unknown }

export const knowledgeModule: SimulatorModule = {
    name: "knowledge",
    description: "Atom acceptance, flashcard reviews, knowledge health",
    dependencies: ["para", "notes"],

    async seed() {
        // No-op: knowledge entities are created by the platform
        // in response to note creation (atom extraction pipeline).
        console.log(`  (knowledge atoms created by server-side extraction)`);
    },

    async simulateDay(world, client, day) {
        // Only review on Tuesday (1), Thursday (3), Saturday (5)
        if (day.dayOfWeek !== 1 && day.dayOfWeek !== 3 && day.dayOfWeek !== 5) {
            return;
        }

        // Find study notes to pull atoms from
        const studyNoteKeys = [...world.createdNotes.entries()]
            .filter(([key]) =>
                key.includes("oauth") ||
                key.includes("french") ||
                key.includes("study")
            );

        let atomsAccepted = 0;

        for (const [, note] of studyNoteKeys) {
            // Fetch atoms for this note — may be empty if extraction hasn't run
            let atoms: AtomResponse[] = [];
            try {
                const result = await client.post<AtomResponse[]>("atoms_for_note", {
                    noteId: note.id,
                });
                atoms = Array.isArray(result) ? result : [];
            } catch {
                // atoms_for_note may fail if no atoms exist yet
                continue;
            }

            if (atoms.length === 0) continue;

            // Accept pending atoms (up to 5 per note)
            const pendingAtoms = atoms.filter(a => a.status === "pending").slice(0, 5);
            for (const atom of pendingAtoms) {
                try {
                    await client.post("atom_accept", {
                        atomId: atom.id,
                        personalImportance: 0.7,
                    });
                    atomsAccepted++;
                } catch {
                    // Atom may already be accepted
                }
            }
        }

        // Try to generate flashcards (skipped in fast mode)
        const oauthNote = world.createdNotes.get("oauth-patterns");
        if (oauthNote) {
            await client.maybe("flashcard_generate", {
                noteId: oauthNote.id,
            });
        }

        // Check knowledge health
        try {
            await client.post("knowledge_health_summary");
        } catch {
            // May not be available if no data exists
        }

        if (atomsAccepted > 0) {
            console.log(`  knowledge: accepted ${atomsAccepted} atoms, reviewed health`);
        } else {
            console.log(`  knowledge: no pending atoms found (extraction may not have run)`);
        }
    },
};
