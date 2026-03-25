// tools/simulator/modules/chat.ts
import type { SimulatorModule } from "./types";

export const chatModule: SimulatorModule = {
    name: "chat",
    description: "Conversations with tool calls (placeholder — requires agent runtime)",
    dependencies: ["para", "tasks", "finance", "notes"],

    async seed() {
        console.log(`  (chat seeding skipped — requires running agent runtime)`);
    },

    async simulateDay() {
        // Phase 2: simulate chat_send with natural language queries
        // that trigger tool calls against the seeded data.
        // Requires the agent runtime to be running alongside the dev server.
    },
};
