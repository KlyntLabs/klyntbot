// tools/simulator/modules/index.ts
import type { SimulatorModule } from "./types";
import { paraModule } from "./para";
import { tasksModule } from "./tasks";
import { financeModule } from "./finance";
import { notesModule } from "./notes";
import { productivityModule } from "./productivity";
import { knowledgeModule } from "./knowledge";
import { cognitiveModule } from "./cognitive";
import { chatModule } from "./chat";

export { type SimulatorModule } from "./types";

/** All available simulator modules. Add new modules here. */
export const ALL_MODULES: SimulatorModule[] = [
    paraModule,
    tasksModule,
    financeModule,
    notesModule,
    productivityModule,
    knowledgeModule,
    cognitiveModule,
    chatModule,
];
