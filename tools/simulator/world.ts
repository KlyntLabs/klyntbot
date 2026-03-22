// tools/simulator/world.ts

export interface Ref {
    id: string;
    title: string;
}

export interface World {
    weekStart: Date;

    // PARA hierarchy (populated by para module)
    areas: {
        personal: Ref;
        work: Ref;
        finance: Ref;
    };
    projects: {
        apiRedesign: Ref;
        parisTrip: Ref;
        fireGoal: Ref;
        languageLearning: Ref;
    };
    objectives: Map<string, Ref>;

    // Finance (populated by finance module)
    accounts: {
        checking: Ref;
        savings: Ref;
        creditCard: Ref;
        brokerage: Ref;
    };

    // Notes (populated by notes module)
    notebooks: {
        workResearch: Ref;
        studyNotes: Ref;
        dailyJournal: Ref;
    };

    // Accumulated across modules — keyed by semantic name (e.g., "auth-meeting-notes")
    createdNotes: Map<string, Ref>;
    createdTasks: Map<string, Ref>;
}

/** Create an empty World shell. Modules populate it during seed(). */
export function createWorld(weekStart: Date): World {
    // Each slot gets its own object to avoid shared-reference mutation bugs.
    const empty = (): Ref => ({ id: "", title: "" });
    return {
        weekStart,
        areas: { personal: empty(), work: empty(), finance: empty() },
        projects: { apiRedesign: empty(), parisTrip: empty(), fireGoal: empty(), languageLearning: empty() },
        objectives: new Map(),
        accounts: { checking: empty(), savings: empty(), creditCard: empty(), brokerage: empty() },
        notebooks: { workResearch: empty(), studyNotes: empty(), dailyJournal: empty() },
        createdNotes: new Map(),
        createdTasks: new Map(),
    };
}
