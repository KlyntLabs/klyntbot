// Single source of truth for query keys. Add new domains here, never inline
// raw arrays at callsites — see docs/superpowers/plans/2026-04-26-realtime-
// data-layer-phase-1.md "Type consistency" section for why.
export const qk = {
	tasks: {
		all: () => ["tasks"] as const,
		today: () => ["tasks", "today"] as const,
		byId: (id: string) => ["tasks", "byId", id] as const,
	},
	calendar: {
		all: () => ["calendar"] as const,
		eventsForDate: (date: string) => ["calendar", "events", date] as const,
	},
	focus: {
		all: () => ["focus"] as const,
		status: () => ["focus", "status"] as const,
		todaySessions: () => ["focus", "todaySessions"] as const,
	},
	flashcards: {
		all: () => ["flashcards"] as const,
		dueCount: () => ["flashcards", "dueCount"] as const,
	},
} as const;

type QkType = typeof qk;
type Domain = QkType[keyof QkType];
type Factory = Domain[keyof Domain];
export type QueryKey = ReturnType<Factory>;
