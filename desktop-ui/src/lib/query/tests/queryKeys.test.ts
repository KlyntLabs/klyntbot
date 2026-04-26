import { describe, expect, it } from "vitest";
import { qk } from "../queryKeys";

describe("queryKeys", () => {
	it("tasks.today is stable", () => {
		expect(qk.tasks.today()).toEqual(["tasks", "today"]);
	});

	it("tasks.byId encodes id", () => {
		expect(qk.tasks.byId("abc")).toEqual(["tasks", "byId", "abc"]);
	});

	it("focus.status has no args", () => {
		expect(qk.focus.status()).toEqual(["focus", "status"]);
	});

	it("flashcards.dueCount is namespaced", () => {
		expect(qk.flashcards.dueCount()).toEqual(["flashcards", "dueCount"]);
	});

	it("calendar.eventsForDate encodes date", () => {
		expect(qk.calendar.eventsForDate("2026-04-26")).toEqual([
			"calendar",
			"events",
			"2026-04-26",
		]);
	});

	it("focus.todaySessions is stable", () => {
		expect(qk.focus.todaySessions()).toEqual(["focus", "todaySessions"]);
	});
});
