import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { qk } from "../queryKeys";
import { startTauriEventBridge } from "../tauriEventBridge";

function fakeListenFactory() {
  const subs = new Map<string, (payload: unknown) => void>();
  const listen = vi.fn(async <T = unknown>(event: string, handler: (payload: T) => void) => {
    subs.set(event, handler as (payload: unknown) => void);
    return () => subs.delete(event);
  });
  const fire = (event: string, payload: unknown) => subs.get(event)?.(payload);
  return { listen, fire, subs };
}

describe("tauriEventBridge", () => {
  it("entity:updated{entityKind:'task'} invalidates tasks.all", async () => {
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("entity:updated", { entityKind: "task", id: "t1" });

    expect(spy).toHaveBeenCalledWith({ queryKey: qk.tasks.all() });
    stop();
  });

  it("focus:phase_changed invalidates focus.status", async () => {
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("focus:phase_changed", { phase: "break" });

    expect(spy).toHaveBeenCalledWith({ queryKey: qk.focus.status() });
    stop();
  });

  it("entity:updated with unknown kind invalidates nothing", async () => {
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("entity:updated", { entityKind: "unknownKind", id: "x" });

    expect(spy).not.toHaveBeenCalled();
    stop();
  });

  it("returns a cleanup that unsubscribes all events", async () => {
    const client = new QueryClient();
    const { listen, subs, fire } = fakeListenFactory();
    const spy = vi.spyOn(client, "invalidateQueries");

    const stop = await startTauriEventBridge(client, listen as any);
    expect(subs.size).toBeGreaterThan(0);
    stop();
    expect(subs.size).toBe(0);

    fire("entity:updated", { entityKind: "task", id: "t1" });
    expect(spy).not.toHaveBeenCalled();
  });
});

it("chat:thread_created invalidates threads.list", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("chat:thread_created", { id: "t1" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
  stop();
});

it("chat:thread_updated invalidates threads.list", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("chat:thread_updated", { id: "t1" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
  stop();
});

it("chat:thread_deleted invalidates threads.all (covers list + byId)", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("chat:thread_deleted", { id: "t1" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.all() });
  stop();
});

it("chat:message_added invalidates threads.list", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("chat:message_added", { sessionKey: "s1" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.threads.list() });
  stop();
});

it("mcp:server_status invalidates system.mcpServers", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("mcp:server_status", { serverName: "x", status: "ready" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.system.mcpServers() });
  stop();
});

it("mcp:startup_complete invalidates system.mcpServers", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("mcp:startup_complete", {});
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.system.mcpServers() });
  stop();
});

it("score:updated invalidates launcher.dashboard", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("score:updated", { score: 0.8 });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dashboard() });
  stop();
});

it("bucket:completed invalidates launcher.dashboard", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("bucket:completed", { bucket: "x" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dashboard() });
  stop();
});

it("focus:state_changed invalidates dndActive too", async () => {
  const client = new QueryClient();
  const spy = vi.spyOn(client, "invalidateQueries");
  const { listen, fire } = fakeListenFactory();
  const stop = await startTauriEventBridge(client, listen as any);
  fire("focus:state_changed", { state: "active" });
  expect(spy).toHaveBeenCalledWith({ queryKey: qk.launcher.dndActive() });
  stop();
});


describe("coding memory + data_version", () => {
  it("entity:updated{kind:'codingFact'} invalidates codingMemory.all()", async () => {
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("entity:updated", { entityKind: "codingFact", id: "fact-1" });

    expect(spy).toHaveBeenCalledWith({ queryKey: qk.codingMemory.all() });
    stop();
  });

  it("entity:updated{kind:'codingEpisode'} invalidates codingMemory.all()", async () => {
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("entity:updated", { entityKind: "codingEpisode", id: "ep-1" });

    expect(spy).toHaveBeenCalledWith({ queryKey: qk.codingMemory.all() });
    stop();
  });

  it("data:version_bumped triggers a broad invalidate (no key prefix)", async () => {
    const client = new QueryClient();
    // Seed two unrelated queries so we can prove BOTH refetch.
    client.setQueryData(qk.tasks.today(), [{ id: "t1" }]);
    client.setQueryData(qk.codingMemory.facts(), [{ id: "f1" }]);
    const spy = vi.spyOn(client, "invalidateQueries");
    const { listen, fire } = fakeListenFactory();

    const stop = await startTauriEventBridge(client, listen as any);
    fire("data:version_bumped", { previous: 41, current: 42 });

    // Broad invalidate: called with no queryKey filter.
    expect(spy).toHaveBeenCalledWith();
    stop();
  });
});
