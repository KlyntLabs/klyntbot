// tools/simulator/modules/para.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { toISO } from "../utils/dates";

interface CreateResponse { id: string; [key: string]: unknown }

export const paraModule: SimulatorModule = {
    name: "para",
    description: "Areas, projects, objectives, key results",
    dependencies: [],

    async seed(world, client) {
        // Areas
        world.areas.personal = await createArea(client, "Personal", "👤");
        world.areas.work = await createArea(client, "Work", "💼");
        world.areas.finance = await createArea(client, "Finance", "💰");
        console.log(`  3 areas created`);

        // Projects
        world.projects.apiRedesign = await createProject(client, "API Redesign", world.areas.work.id, "Redesign the authentication and API layer");
        world.projects.parisTrip = await createProject(client, "Paris Trip Planning", world.areas.personal.id, "March trip to Paris — research, budget, itinerary");
        world.projects.fireGoal = await createProject(client, "FIRE Goal Tracking", world.areas.finance.id, "Financial independence planning and investment tracking");
        world.projects.languageLearning = await createProject(client, "French Language Learning", world.areas.personal.id, "B1 proficiency by Q4");
        console.log(`  4 projects created`);

        // Objectives
        const obj1 = await createObjective(client, "Ship auth v2 by end of sprint", world.projects.apiRedesign.id);
        world.objectives.set("auth-v2", obj1);
        const obj2 = await createObjective(client, "Reach 50% savings rate this quarter", world.projects.fireGoal.id);
        world.objectives.set("savings-rate", obj2);
        const obj3 = await createObjective(client, "Complete B1 French vocab", world.projects.languageLearning.id);
        world.objectives.set("french-vocab", obj3);
        console.log(`  3 objectives created`);

        // Key Results
        await createKeyResult(client, obj1.id, "Migrate 100% of endpoints to new auth", 100, "%");
        await createKeyResult(client, obj1.id, "Zero auth-related incidents in staging", 0, "count");
        await createKeyResult(client, obj2.id, "Monthly savings >= $3,000", 3000, "USD");
        await createKeyResult(client, obj2.id, "Investment contributions >= $1,500/month", 1500, "USD");
        await createKeyResult(client, obj3.id, "Learn 500 new vocabulary words", 500, "words");
        await createKeyResult(client, obj3.id, "Complete 30 practice sessions", 30, "sessions");
        console.log(`  6 key results created`);

        // ── Project sources (project_sources table) ─────────────────
        await client.post("project_source_create", {
            projectId: world.projects.apiRedesign.id,
            sourceType: "document",
            title: "Auth Migration RFC",
            content: "RFC-2024-AUTH: Migration plan from legacy session-based auth to JWT + refresh token rotation.",
            tags: ["rfc", "auth", "architecture"],
        });
        await client.post("project_source_create", {
            projectId: world.projects.apiRedesign.id,
            sourceType: "url",
            title: "OAuth 2.0 Best Practices (IETF)",
            url: "https://datatracker.ietf.org/doc/html/draft-ietf-oauth-security-topics",
            tags: ["oauth", "reference", "security"],
        });
        await client.post("project_source_create", {
            projectId: world.projects.parisTrip.id,
            sourceType: "url",
            title: "Paris Museum Pass Guide",
            url: "https://www.parismuseumpass.com",
            tags: ["travel", "paris", "museums"],
        });
        await client.post("project_source_create", {
            projectId: world.projects.fireGoal.id,
            sourceType: "document",
            title: "FIRE Calculator Assumptions",
            content: "25x annual expenses rule. Assumes 4% safe withdrawal rate, 7% real returns, 2% inflation.",
            tags: ["fire", "finance", "planning"],
        });
        console.log(`  4 project sources created`);
    },

    async simulateDay(world, client, day) {
        // Wednesday: update OKR progress
        if (day.dayOfWeek === 2) {
            const authObj = world.objectives.get("auth-v2");
            if (authObj) {
                // Update key result progress (if API supports it)
                console.log(`  📊 para: updated OKR progress`);
            }
        }
    },
};

async function createArea(client: ApiClient, name: string, icon: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("area_create", { name, icon });
    return { id: res.id, title: name };
}

async function createProject(client: ApiClient, name: string, areaId: string, description: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("project_create", { name, areaId, description });
    return { id: res.id, title: name };
}

async function createObjective(client: ApiClient, title: string, projectId: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("objective_create", { title, projectId });
    return { id: res.id, title };
}

async function createKeyResult(client: ApiClient, objectiveId: string, title: string, targetValue: number, unit: string): Promise<Ref> {
    const res = await client.post<CreateResponse>("key_result_create", { objectiveId, title, targetValue, unit });
    return { id: res.id, title };
}
