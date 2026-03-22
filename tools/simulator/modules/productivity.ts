// tools/simulator/modules/productivity.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { formatDate } from "../utils/dates";
import { pick, randomBetween } from "../utils/random";

export const productivityModule: SimulatorModule = {
    name: "productivity",
    description: "Focus sessions, time entries, goals",
    dependencies: ["para", "tasks"],

    async seed(world, client) {
        // Create productivity goals using postFlat (snake_case, flat body)
        await client.postFlat("productivity_goal_create", {
            goal_type: "daily",
            metric: "focus_minutes",
            target_value: 240, // 4 hours deep work
        });
        await client.postFlat("productivity_goal_create", {
            goal_type: "weekly",
            metric: "tasks_completed",
            target_value: 5,
        });
        await client.postFlat("productivity_goal_create", {
            goal_type: "daily",
            metric: "focus_sessions",
            target_value: 3,
        });
        console.log(`  3 productivity goals created`);
    },

    async simulateDay(world, client, day) {
        const date = formatDate(day.date);
        const taskEntries = [...world.createdTasks.entries()];

        if (day.isWeekend) {
            // 0-1 focus sessions on weekends
            if (randomBetween(0, 1) === 1 && taskEntries.length > 0) {
                const [, task] = pick(taskEntries);
                await runFocusSession(client, task.id, randomBetween(30, 60));
                console.log(`  productivity: 1 weekend focus session`);
            } else {
                console.log(`  productivity: rest day, no sessions`);
            }
            return;
        }

        // Weekday: 2-3 focus sessions
        const sessionCount = randomBetween(2, 3);
        let totalMinutes = 0;

        // Pick distinct tasks for sessions
        const sessionTasks = taskEntries.length >= sessionCount
            ? taskEntries.slice(0, sessionCount)
            : taskEntries;

        for (let i = 0; i < Math.min(sessionCount, sessionTasks.length); i++) {
            const [, task] = sessionTasks[i];
            const duration = pick([45, 60, 90, 120]);
            await runFocusSession(client, task.id, duration);
            totalMinutes += duration;
        }

        // Log a manual time entry for meetings / non-focus work
        const meetingMins = randomBetween(30, 90);
        await client.postFlat("productivity_time_entry_create", {
            description: `Meetings and sync calls (${date})`,
            duration_mins: meetingMins,
            project_id: world.projects.apiRedesign.id,
        });
        totalMinutes += meetingMins;

        // On Monday, also log planning time
        if (day.dayOfWeek === 0) {
            await client.postFlat("productivity_time_entry_create", {
                description: "Weekly planning and task review",
                duration_mins: 30,
            });
            totalMinutes += 30;
        }

        console.log(`  productivity: ${sessionCount} focus sessions, ${totalMinutes}min total tracked`);
    },
};

async function runFocusSession(client: ApiClient, taskId: string, targetMins: number): Promise<void> {
    // Start session
    await client.postFlat("productivity_focus_start", {
        action_id: taskId,
        target_mins: targetMins,
    });

    // End session (in a real sim we'd wait, but we just call end immediately)
    await client.postFlat("productivity_focus_end", {
        notes: `Focused for ~${targetMins}min`,
    });
}
