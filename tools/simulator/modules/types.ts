// tools/simulator/modules/types.ts
import type { World } from "../world";
import type { ApiClient } from "../client";

export interface DayContext {
    date: Date;
    /** Simulator-internal ordinal: 0=Monday, 6=Sunday. NOT Date.getDay(). */
    dayOfWeek: number;
    isWeekend: boolean;
    /** 0-based index within the simulation run. */
    dayIndex: number;
}

export interface SimulatorModule {
    name: string;
    description: string;
    /** Module names that must run seed() before this one. */
    dependencies: string[];
    /** Create structural entities this module owns. Fatal on failure. */
    seed(world: World, client: ApiClient): Promise<void>;
    /** Simulate a day of activity. Non-fatal on failure (logged, continues). */
    simulateDay(world: World, client: ApiClient, day: DayContext): Promise<void>;
}
