// tools/simulator/modules/cognitive.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";

interface FactResponse { id: string; [key: string]: unknown }

const BASE_FACTS: { domain: string; subject: string; predicate: string; object: string; confidence: number }[] = [
    { domain: "identity", subject: "user", predicate: "works_as", object: "software engineer", confidence: 0.95 },
    { domain: "identity", subject: "user", predicate: "lives_in", object: "San Francisco", confidence: 0.9 },
    { domain: "learning", subject: "user", predicate: "is_learning", object: "French language (targeting B1)", confidence: 0.9 },
    { domain: "work", subject: "user", predicate: "current_project", object: "API Redesign — auth layer migration", confidence: 0.85 },
    { domain: "finance", subject: "user", predicate: "financial_goal", object: "FIRE — targeting 50% savings rate", confidence: 0.85 },
    { domain: "preferences", subject: "user", predicate: "preferred_work_style", object: "deep work blocks of 90+ minutes", confidence: 0.8 },
    { domain: "preferences", subject: "user", predicate: "preferred_tools", object: "Rust, TypeScript, Neovim", confidence: 0.8 },
    { domain: "identity", subject: "user", predicate: "planning_trip_to", object: "Paris in March", confidence: 0.85 },
    { domain: "energy", subject: "user", predicate: "peak_productivity", object: "mornings (9am-12pm)", confidence: 0.75 },
    { domain: "preferences", subject: "user", predicate: "coffee_preference", object: "oat milk latte, no sugar", confidence: 0.7 },
];

const DAILY_EPISODIC_TEMPLATES: Record<number, (world: World) => { domain: string; content: string }> = {
    0: (world) => ({
        domain: "work",
        content: `Productive Monday: reviewed tasks for ${world.projects.apiRedesign.title}, had auth design meeting with Sarah and Mike. Made progress on JWT implementation.`,
    }),
    1: (world) => ({
        domain: "learning",
        content: `Research day: studied OAuth 2.0 patterns for the API redesign. Also reviewed Paris trip budget — looking at $3,500-$4,500 total.`,
    }),
    2: (world) => ({
        domain: "work",
        content: `Sprint retrospective for ${world.projects.apiRedesign.title}. Good progress on auth — 85% test coverage. Need to improve staging deployment automation.`,
    }),
    3: (world) => ({
        domain: "learning",
        content: `Deep study session on OAuth security patterns. Created detailed study notes. French vocabulary practice — learned 5 new words.`,
    }),
    4: (world) => ({
        domain: "work",
        content: `Friday wrap-up: completed several tasks, logged Paris trip expenses. Weekly reflection — need more discipline on deep work blocks.`,
    }),
    5: (world) => ({
        domain: "learning",
        content: `Weekend study: French vocab practice, reviewed grammar notes on subjunctive mood. Light personal planning.`,
    }),
    6: (world) => ({
        domain: "preferences",
        content: `Sunday planning: reviewed upcoming week, set intentions for auth migration completion and FIRE goal check-in. Feeling prepared for the week ahead.`,
    }),
};

export const cognitiveModule: SimulatorModule = {
    name: "cognitive",
    description: "Semantic facts, episodic memories, procedural rules",
    dependencies: ["para", "tasks", "notes"],

    async seed(world, client) {
        let count = 0;
        for (const fact of BASE_FACTS) {
            await client.post<FactResponse>("cognitive_fact_create", {
                domain: fact.domain,
                subject: fact.subject,
                predicate: fact.predicate,
                object: fact.object,
                confidence: fact.confidence,
            });
            count++;
        }
        console.log(`  ${count} semantic facts created`);
    },

    async simulateDay(world, client, day) {
        // Inject daily activity as UserStatedFact events
        // Valid event types: UserStatedFact, UserCorrectedAI, BudgetAlert,
        // DistractionDetected, FocusSessionStarted, FocusSessionEnded, TaskDeferred
        const template = DAILY_EPISODIC_TEMPLATES[day.dayOfWeek];
        if (template) {
            const episodic = template(world);
            await client.postFlat("cognitive_inject_event", {
                event_type: "UserStatedFact",
                payload: {
                    fact: episodic.content,
                    domain: episodic.domain,
                },
            });
            console.log(`  cognitive: injected ${episodic.domain} fact`);
        }

        // On Wednesday, inject a distraction event for realism
        if (day.dayOfWeek === 2) {
            await client.postFlat("cognitive_inject_event", {
                event_type: "DistractionDetected",
                payload: {
                    app: "Twitter",
                    duration_secs: 300,
                    context: "Browsed during focus session on auth implementation",
                },
            });
            console.log(`  cognitive: injected distraction event`);
        }

        // On Friday, inject a budget alert
        if (day.dayOfWeek === 4) {
            await client.postFlat("cognitive_inject_event", {
                event_type: "BudgetAlert",
                payload: {
                    category: "dining",
                    spent: 280.0,
                    limit: 300.0,
                },
            });
            console.log(`  cognitive: injected budget alert event`);
        }
    },
};
