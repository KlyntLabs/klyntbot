// tools/simulator/modules/tasks.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { formatDate } from "../utils/dates";
import { pick, randomBetween, shuffle } from "../utils/random";

interface CreateResponse { id: string; [key: string]: unknown }

const TASK_DEFINITIONS: { key: string; title: string; project: keyof World["projects"]; priority?: number; tags?: string[]; estimatedMinutes?: number }[] = [
    // API Redesign tasks
    { key: "auth-layer", title: "Implement auth layer", project: "apiRedesign", priority: 1, tags: ["backend", "auth"], estimatedMinutes: 240 },
    { key: "jwt-refresh", title: "Add JWT refresh token rotation", project: "apiRedesign", priority: 1, tags: ["backend", "security"], estimatedMinutes: 180 },
    { key: "rate-limiting", title: "Implement rate limiting middleware", project: "apiRedesign", priority: 2, tags: ["backend", "infra"], estimatedMinutes: 120 },
    { key: "api-docs", title: "Write OpenAPI documentation", project: "apiRedesign", priority: 3, tags: ["docs"], estimatedMinutes: 90 },
    { key: "api-tests", title: "Integration tests for auth endpoints", project: "apiRedesign", priority: 2, tags: ["testing"], estimatedMinutes: 150 },
    { key: "api-migration", title: "Migrate legacy endpoints to v2", project: "apiRedesign", priority: 2, tags: ["backend", "migration"], estimatedMinutes: 300 },
    { key: "api-monitoring", title: "Set up API monitoring dashboards", project: "apiRedesign", priority: 3, tags: ["infra", "observability"], estimatedMinutes: 60 },

    // Paris Trip tasks
    { key: "paris-flights", title: "Research and book flights to Paris", project: "parisTrip", priority: 1, tags: ["travel", "booking"], estimatedMinutes: 60 },
    { key: "paris-hotel", title: "Book hotel near Marais district", project: "parisTrip", priority: 1, tags: ["travel", "booking"], estimatedMinutes: 45 },
    { key: "paris-itinerary", title: "Create day-by-day itinerary", project: "parisTrip", priority: 2, tags: ["travel", "planning"], estimatedMinutes: 90 },
    { key: "paris-budget", title: "Set Paris trip budget and alerts", project: "parisTrip", priority: 2, tags: ["travel", "finance"], estimatedMinutes: 30 },
    { key: "paris-restaurants", title: "Research restaurants and cafes", project: "parisTrip", priority: 3, tags: ["travel", "food"], estimatedMinutes: 45 },

    // FIRE Goal tasks
    { key: "fire-review", title: "Review monthly investment allocations", project: "fireGoal", priority: 1, tags: ["finance", "investing"], estimatedMinutes: 30 },
    { key: "fire-rebalance", title: "Rebalance portfolio quarterly", project: "fireGoal", priority: 2, tags: ["finance", "investing"], estimatedMinutes: 45 },
    { key: "fire-tax", title: "Research tax-advantaged accounts", project: "fireGoal", priority: 2, tags: ["finance", "tax"], estimatedMinutes: 60 },

    // Language Learning tasks
    { key: "french-vocab", title: "Complete Anki French vocab deck", project: "languageLearning", priority: 2, tags: ["learning", "french"], estimatedMinutes: 30 },
    { key: "french-grammar", title: "Study subjunctive mood grammar", project: "languageLearning", priority: 2, tags: ["learning", "french"], estimatedMinutes: 45 },
    { key: "french-podcast", title: "Listen to InnerFrench podcast episodes", project: "languageLearning", priority: 3, tags: ["learning", "french", "listening"], estimatedMinutes: 30 },
    { key: "french-writing", title: "Write 3 journal entries in French", project: "languageLearning", priority: 3, tags: ["learning", "french", "writing"], estimatedMinutes: 60 },
];

export const tasksModule: SimulatorModule = {
    name: "tasks",
    description: "Task lifecycle — creation, completion, updates",
    dependencies: ["para"],

    async seed(world, client) {
        let count = 0;
        for (const def of TASK_DEFINITIONS) {
            const project = world.projects[def.project];
            const res = await client.post<CreateResponse>("task_create", {
                title: def.title,
                projectId: project.id,
                priority: def.priority ?? null,
                tags: def.tags ?? [],
                estimatedMinutes: def.estimatedMinutes ?? null,
            });
            world.createdTasks.set(def.key, { id: res.id, title: def.title });
            count++;
        }
        console.log(`  ${count} tasks created across ${Object.keys(world.projects).length} projects`);
    },

    async simulateDay(world, client, day) {
        if (day.isWeekend) {
            // Light weekend: maybe toggle one task
            const tasks = [...world.createdTasks.values()];
            if (tasks.length > 0) {
                const task = pick(tasks);
                await client.post("today_tasks");
                console.log(`  tasks: checked today view`);
            }
            return;
        }

        // Weekday activity
        const tasks = [...world.createdTasks.entries()];
        const activeTasks = shuffle(tasks);

        // Complete 1-2 tasks
        const completeCount = randomBetween(1, 2);
        let completed = 0;
        for (let i = 0; i < Math.min(completeCount, activeTasks.length); i++) {
            const [key, task] = activeTasks[i];
            await client.post("task_toggle_complete", { id: task.id });
            completed++;
        }

        // Create 1-2 new tasks
        const newTaskCount = randomBetween(1, 2);
        const projectKeys = Object.keys(world.projects) as (keyof World["projects"])[];
        const newTitles = [
            "Review PR feedback on auth changes",
            "Update error handling for edge cases",
            "Fix CORS configuration for staging",
            "Add input validation to API endpoints",
            "Schedule sync with design team",
            "Update project dependencies",
            "Write unit tests for token service",
            "Benchmark API response times",
            "Draft weekly progress report",
            "Clean up unused feature flags",
            "Investigate flaky CI test",
            "Update README with new API examples",
            "Set up staging environment variables",
            "Review security audit findings",
        ];
        const dayTitle = newTitles[day.dayIndex % newTitles.length];
        const projectKey = projectKeys[day.dayIndex % projectKeys.length];
        const project = world.projects[projectKey];

        for (let i = 0; i < newTaskCount; i++) {
            const title = i === 0 ? dayTitle : `Follow-up: ${dayTitle}`;
            const res = await client.post<CreateResponse>("task_create", {
                title,
                projectId: project.id,
                tags: ["daily"],
            });
            const taskKey = `day${day.dayIndex}-task${i}`;
            world.createdTasks.set(taskKey, { id: res.id, title });
        }

        // Update a task (add description or change priority)
        if (activeTasks.length > 2) {
            const [, taskToUpdate] = activeTasks[2];
            await client.post("task_update", {
                id: taskToUpdate.id,
                description: `Updated during day ${day.dayIndex + 1} review`,
            });
        }

        // Check today view
        await client.post("today_tasks");

        console.log(`  tasks: completed ${completed}, created ${newTaskCount}, updated 1`);
    },
};
