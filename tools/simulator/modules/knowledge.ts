// tools/simulator/modules/knowledge.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { pick, randomBetween } from "../utils/random";

interface AtomResponse { id: string; status: string; [key: string]: unknown }
interface FlashcardResponse { id: string; deck: string; [key: string]: unknown }

export const knowledgeModule: SimulatorModule = {
    name: "knowledge",
    description: "Atom acceptance, flashcard reviews, knowledge health",
    dependencies: ["para", "notes"],

    async seed(_world, client) {
        // Create French Vocabulary flashcards
        await client.post("flashcard_create", {
            deck: "French Vocabulary",
            front: "Bonjour",
            back: "Hello / Good morning",
            cardType: "basic",
            tags: ["french", "greetings"],
        });
        await client.post("flashcard_create", {
            deck: "French Vocabulary",
            front: "Merci beaucoup",
            back: "Thank you very much",
            cardType: "basic",
            tags: ["french", "politeness"],
        });
        await client.post("flashcard_create", {
            deck: "French Vocabulary",
            front: "Comment allez-vous?",
            back: "How are you? (formal)",
            cardType: "basic",
            tags: ["french", "greetings"],
        });
        await client.post("flashcard_create", {
            deck: "French Vocabulary",
            front: "S'il vous plait",
            back: "Please (formal)",
            cardType: "basic",
            tags: ["french", "politeness"],
        });
        await client.post("flashcard_create", {
            deck: "French Vocabulary",
            front: "Au revoir",
            back: "Goodbye",
            cardType: "basic",
            tags: ["french", "greetings"],
        });

        // Create OAuth Concepts flashcards
        await client.post("flashcard_create", {
            deck: "OAuth Concepts",
            front: "What is PKCE?",
            back: "Proof Key for Code Exchange — prevents authorization code interception attacks in public clients",
            cardType: "basic",
            tags: ["oauth", "security"],
        });
        await client.post("flashcard_create", {
            deck: "OAuth Concepts",
            front: "What is the difference between an access token and a refresh token?",
            back: "Access tokens are short-lived (15-30 min) and used for API requests. Refresh tokens are long-lived and used to obtain new access tokens.",
            cardType: "basic",
            tags: ["oauth", "tokens"],
        });
        await client.post("flashcard_create", {
            deck: "OAuth Concepts",
            front: "What are OAuth 2.0 scopes?",
            back: "Scopes define the specific permissions granted to an application — they limit what the access token can do.",
            cardType: "basic",
            tags: ["oauth", "authorization"],
        });
        await client.post("flashcard_create", {
            deck: "OAuth Concepts",
            front: "Why use token rotation for refresh tokens?",
            back: "Token rotation invalidates old refresh tokens when a new one is issued, limiting the window of attack if a token is stolen.",
            cardType: "basic",
            tags: ["oauth", "security"],
        });
        await client.post("flashcard_create", {
            deck: "OAuth Concepts",
            front: "What is the Authorization Code flow?",
            back: "Most secure OAuth flow for server-side apps: user authenticates at auth server, receives code, client exchanges code for tokens server-to-server.",
            cardType: "basic",
            tags: ["oauth", "flows"],
        });
        console.log(`  10 flashcards created across 2 decks`);

        // Set deck preferences
        try {
            await client.postFlat("flashcard_save_mode_preference", {
                deck: "French Vocabulary",
                mode: "standard",
            });
            await client.postFlat("flashcard_save_mode_preference", {
                deck: "OAuth Concepts",
                mode: "standard",
            });
            console.log(`  2 deck preferences saved`);
        } catch {
            // Deck preference saving may fail if repo not initialized
        }
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

        // Review flashcards — fetch due cards and record reviews
        let cardsReviewed = 0;
        const decks = ["French Vocabulary", "OAuth Concepts"];
        for (const deck of decks) {
            try {
                const cards = await client.postFlat<FlashcardResponse[]>("flashcard_get_due", {
                    deck,
                    limit: 5,
                });
                const dueCards = Array.isArray(cards) ? cards : [];
                for (const card of dueCards) {
                    try {
                        await client.post("flashcard_record_review", {
                            cardId: card.id,
                            quality: pick(["good", "easy", "hard"]),
                            recallSpeedMs: randomBetween(1000, 5000),
                        });
                        cardsReviewed++;
                    } catch {
                        // Review recording may fail
                    }
                }
            } catch {
                // Deck may not have due cards
            }
        }
        if (cardsReviewed > 0) {
            console.log(`  knowledge: reviewed ${cardsReviewed} flashcards`);
        }

        // Check knowledge health
        try {
            await client.post("knowledge_health_summary");
        } catch {
            // May not be available if no data exists
        }

        if (atomsAccepted > 0 || cardsReviewed > 0) {
            console.log(`  knowledge: accepted ${atomsAccepted} atoms, reviewed ${cardsReviewed} cards`);
        } else {
            console.log(`  knowledge: no pending atoms found (extraction may not have run)`);
        }
    },
};
