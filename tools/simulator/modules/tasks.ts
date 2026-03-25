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
        // Map projects to their areas for areaId (required for top-level tasks)
        const projectArea: Record<string, Ref> = {
            apiRedesign: world.areas.work,
            parisTrip: world.areas.personal,
            fireGoal: world.areas.finance,
            languageLearning: world.areas.personal,
        };

        let count = 0;
        for (const def of TASK_DEFINITIONS) {
            const project = world.projects[def.project];
            const area = projectArea[def.project];
            const res = await client.post<CreateResponse>("task_create", {
                title: def.title,
                projectId: project.id,
                areaId: area.id,
                priority: def.priority ?? null,
                tags: def.tags ?? [],
                estimatedMinutes: def.estimatedMinutes ?? null,
            });
            world.createdTasks.set(def.key, { id: res.id, title: def.title });
            count++;
        }
        console.log(`  ${count} tasks created across ${Object.keys(world.projects).length} projects`);

        // Create task groups
        await client.post<CreateResponse>("group_create", {
            projectId: world.projects.apiRedesign.id,
            name: "Sprint 1",
            color: "#3B82F6",
        });
        await client.post<CreateResponse>("group_create", {
            projectId: world.projects.apiRedesign.id,
            name: "Sprint 2",
            color: "#10B981",
        });
        await client.post<CreateResponse>("group_create", {
            projectId: world.projects.parisTrip.id,
            name: "Before Trip",
            color: "#F59E0B",
        });
        console.log(`  3 task groups created`);

        // Create custom columns
        const complexityCol = await client.post<CreateResponse>("custom_column_create", {
            projectId: world.projects.apiRedesign.id,
            name: "Complexity",
            columnType: "dropdown",
            options: ["low", "medium", "high"],
        });
        const storyPointsCol = await client.post<CreateResponse>("custom_column_create", {
            projectId: world.projects.apiRedesign.id,
            name: "Story Points",
            columnType: "number",
        });
        console.log(`  2 custom columns created`);

        // Set custom column values on a few tasks
        const authLayer = world.createdTasks.get("auth-layer");
        const jwtRefresh = world.createdTasks.get("jwt-refresh");
        const rateLimiting = world.createdTasks.get("rate-limiting");
        const apiDocs = world.createdTasks.get("api-docs");
        const apiTests = world.createdTasks.get("api-tests");
        const apiMigration = world.createdTasks.get("api-migration");
        if (authLayer && complexityCol?.id) {
            await client.post("custom_column_value_set", {
                taskId: authLayer.id,
                columnId: complexityCol.id,
                valueJson: JSON.stringify("high"),
            });
            await client.post("custom_column_value_set", {
                taskId: authLayer.id,
                columnId: storyPointsCol.id,
                valueJson: JSON.stringify(8),
            });
        }
        if (jwtRefresh && complexityCol?.id) {
            await client.post("custom_column_value_set", {
                taskId: jwtRefresh.id,
                columnId: complexityCol.id,
                valueJson: JSON.stringify("high"),
            });
        }

        // Task dependencies: jwt-refresh blocked by auth-layer, api-tests blocked by jwt-refresh
        if (authLayer && jwtRefresh) {
            await client.postFlat("task_add_dependency", { taskId: jwtRefresh.id, blockerId: authLayer.id });
        }
        if (jwtRefresh && apiTests) {
            await client.postFlat("task_add_dependency", { taskId: apiTests.id, blockerId: jwtRefresh.id });
        }
        if (rateLimiting && apiMigration) {
            await client.postFlat("task_add_dependency", { taskId: apiMigration.id, blockerId: rateLimiting.id });
        }
        console.log(`  3 task dependencies created`);

        // Task attachments
        if (authLayer) {
            await client.postFlat("task_add_attachment", {
                taskId: authLayer.id,
                attachmentType: "link",
                value: "https://datatracker.ietf.org/doc/html/rfc7519",
                title: "JWT RFC 7519",
            });
        }
        if (apiDocs) {
            await client.postFlat("task_add_attachment", {
                taskId: apiDocs.id,
                attachmentType: "link",
                value: "https://swagger.io/specification/",
                title: "OpenAPI 3.1 Spec",
            });
        }
        console.log(`  2 task attachments created`);

        // Task time entries
        const weekStartIso = world.weekStart.toISOString();
        if (authLayer) {
            await client.postFlat("task_add_time_entry", {
                taskId: authLayer.id,
                startedAt: weekStartIso,
                durationSecs: 5400,
                note: "Initial auth layer scaffolding",
            });
        }
        if (jwtRefresh) {
            await client.postFlat("task_add_time_entry", {
                taskId: jwtRefresh.id,
                startedAt: weekStartIso,
                durationSecs: 3600,
                note: "Research JWT rotation strategies",
            });
        }
        if (rateLimiting) {
            await client.postFlat("task_add_time_entry", {
                taskId: rateLimiting.id,
                startedAt: weekStartIso,
                durationSecs: 2700,
                note: "Prototype sliding window limiter",
            });
        }
        console.log(`  3 task time entries created`);
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
            await client.postFlat("task_toggle_complete", { id: task.id });
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

        // Resolve area for the selected project
        const projectAreaMap: Record<string, Ref> = {
            apiRedesign: world.areas.work,
            parisTrip: world.areas.personal,
            fireGoal: world.areas.finance,
            languageLearning: world.areas.personal,
        };
        const area = projectAreaMap[projectKey];

        for (let i = 0; i < newTaskCount; i++) {
            const title = i === 0 ? dayTitle : `Follow-up: ${dayTitle}`;
            const res = await client.post<CreateResponse>("task_create", {
                title,
                projectId: project.id,
                areaId: area.id,
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

        // ── Task decomposition (task_decompositions table) ──────────
        // On Wednesday, attempt AI decomposition for a complex task.
        // Skipped in fast mode (already in SKIP_IN_MODE).
        if (day.dayOfWeek === 2 && activeTasks.length > 3) {
            const [, complexTask] = activeTasks[3];
            try {
                const decomp = await client.maybeFlat<{ id: string; subtasks: unknown[] }>(
                    "task_decompose",
                    { taskId: complexTask.id },
                );
                if (decomp?.id) {
                    // Apply the decomposition to create subtasks
                    try {
                        await client.postFlat("task_apply_decomposition", {
                            decompositionId: decomp.id,
                        });
                        console.log(`  tasks: decomposed "${complexTask.title}" into subtasks`);
                    } catch {
                        // Decomposition may fail if task doesn't have enough context
                    }
                }
            } catch {
                // Decomposition requires LLM — may fail in environments without API keys
            }
        }

        console.log(`  tasks: completed ${completed}, created ${newTaskCount}, updated 1`);
    },
};
