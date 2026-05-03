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
    expect(qk.calendar.eventsForDate("2026-04-26")).toEqual(["calendar", "events", "2026-04-26"]);
  });

  it("focus.todaySessions is stable", () => {
    expect(qk.focus.todaySessions()).toEqual(["focus", "todaySessions"]);
  });
});

describe("queryKeys — phase 2 domains", () => {
  it("launcher.dashboard / search / dndActive", () => {
    expect(qk.launcher.dashboard()).toEqual(["launcher", "dashboard"]);
    expect(qk.launcher.search("hi")).toEqual(["launcher", "search", "hi"]);
    expect(qk.launcher.dndActive()).toEqual(["launcher", "dndActive"]);
  });
  it("settings keys", () => {
    expect(qk.settings.app()).toEqual(["settings", "app"]);
    expect(qk.settings.codexConfigPath()).toEqual(["settings", "codexConfigPath"]);
    expect(qk.settings.features("ws-1")).toEqual(["settings", "features", "ws-1"]);
    expect(qk.settings.tailscaleStatus()).toEqual(["settings", "tailscaleStatus"]);
    expect(qk.settings.tailscaleCommandPreview()).toEqual(["settings", "tailscaleCommandPreview"]);
    expect(qk.settings.tcpDaemonStatus()).toEqual(["settings", "tcpDaemonStatus"]);
    expect(qk.settings.workspaces()).toEqual(["settings", "workspaces"]);
  });
  it("agents keys", () => {
    expect(qk.agents.settings()).toEqual(["agents", "settings"]);
    expect(qk.agents.configToml("foo")).toEqual(["agents", "configToml", "foo"]);
  });
  it("models keys", () => {
    expect(qk.models.list("ws-1")).toEqual(["models", "list", "ws-1"]);
    expect(qk.models.configModel("ws-1")).toEqual(["models", "configModel", "ws-1"]);
  });
  it("registries", () => {
    expect(qk.skills.list("ws-1")).toEqual(["skills", "list", "ws-1"]);
    expect(qk.apps.list("ws-1", "thread-7")).toEqual(["apps", "list", "ws-1", "thread-7"]);
    expect(qk.prompts.list("ws-1")).toEqual(["prompts", "list", "ws-1"]);
  });
  it("git keys", () => {
    expect(qk.git.status("ws-1")).toEqual(["git", "status", "ws-1"]);
    expect(qk.git.branches("ws-1")).toEqual(["git", "branches", "ws-1"]);
    expect(qk.git.diffs("ws-1")).toEqual(["git", "diffs", "ws-1"]);
    expect(qk.git.log("ws-1")).toEqual(["git", "log", "ws-1"]);
    expect(qk.git.remote("ws-1")).toEqual(["git", "remote", "ws-1"]);
    expect(qk.git.commitDiffs("ws-1", "abc")).toEqual(["git", "commitDiffs", "ws-1", "abc"]);
    expect(qk.git.repoScan("ws-1", 2)).toEqual(["git", "repoScan", "ws-1", 2]);
  });
  it("github keys", () => {
    expect(qk.github.issues("ws-1")).toEqual(["github", "issues", "ws-1"]);
    expect(qk.github.pulls("ws-1")).toEqual(["github", "pulls", "ws-1"]);
    expect(qk.github.diffsForPr("ws-1", 42)).toEqual(["github", "pulls", "ws-1", 42, "diffs"]);
    expect(qk.github.commentsForPr("ws-1", 42)).toEqual([
      "github",
      "pulls",
      "ws-1",
      42,
      "comments",
    ]);
  });
  it("threads / system", () => {
    expect(qk.threads.list()).toEqual(["threads", "list"]);
    expect(qk.threads.byId("abc")).toEqual(["threads", "byId", "abc"]);
    expect(qk.system.mcpServers()).toEqual(["system", "mcpServers"]);
  });
});

describe("codingMemory keys", () => {
  it("all is the root", () => {
    expect(qk.codingMemory.all()).toEqual(["codingMemory"]);
  });
  it("facts / episodes / recallIndex / memoryBrowser / status are stable", () => {
    expect(qk.codingMemory.facts()).toEqual(["codingMemory", "facts"]);
    expect(qk.codingMemory.episodes()).toEqual(["codingMemory", "episodes"]);
    expect(qk.codingMemory.recallIndex()).toEqual(["codingMemory", "recallIndex"]);
    expect(qk.codingMemory.memoryBrowser()).toEqual(["codingMemory", "memoryBrowser"]);
    expect(qk.codingMemory.status()).toEqual(["codingMemory", "status"]);
  });
});

describe("dashboard keys", () => {
  it("timeline key normalizes source order", () => {
    const a = qk.dashboard.timeline("2026-04-30", "2026-04-30", ["task", "calendar"]);
    const b = qk.dashboard.timeline("2026-04-30", "2026-04-30", ["calendar", "task"]);
    expect(a).toEqual(b);
    expect(a).toEqual(["dashboard", "timeline", "2026-04-30", "2026-04-30", "calendar,task"]);
  });

  it("dashboard.all is the namespace root", () => {
    expect(qk.dashboard.all()).toEqual(["dashboard"]);
  });

  it("calendarSync.status is namespaced", () => {
    expect(qk.calendarSync.status()).toEqual(["calendarSync", "status"]);
  });

  it("productivity.calendarEvents encodes date", () => {
    expect(qk.productivity.calendarEvents("2026-04-30")).toEqual([
      "productivity",
      "calendarEvents",
      "2026-04-30",
    ]);
  });
});
