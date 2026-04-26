import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/services/events", () => ({
  subscribeAppServerEvents: vi.fn(),
}));

import { subscribeAppServerEvents } from "@/services/events";
import { startAppServerEventBridge } from "../appServerEventBridge";
import { qk } from "../queryKeys";

const mockedSubscribe = vi.mocked(subscribeAppServerEvents);

function makeFakeSub() {
  let handler: (e: unknown) => void = () => {};
  const unsubscribe = vi.fn();
  (mockedSubscribe as any).mockImplementation((h: (e: unknown) => void) => {
    handler = h;
    return unsubscribe;
  });
  return { fire: (e: unknown) => handler(e), unsubscribe };
}

describe("appServerEventBridge", () => {
  it("invalidates skills.list when SkillsUpdateAvailable fires", () => {
    const { fire } = makeFakeSub();
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    startAppServerEventBridge(client);
    fire({ type: "SkillsUpdateAvailable", workspaceId: "ws-1" });
    expect(spy).toHaveBeenCalledWith({
      queryKey: qk.skills.list("ws-1"),
    });
  });

  it("invalidates apps.list (broad) when AppListUpdated fires", () => {
    const { fire } = makeFakeSub();
    const client = new QueryClient();
    const spy = vi.spyOn(client, "invalidateQueries");
    startAppServerEventBridge(client);
    fire({ type: "AppListUpdated", workspaceId: "ws-1" });
    // Broad invalidation: prefix `qk.apps.all()` covers every (workspaceId,
    // threadId) variant under "apps".
    expect(spy).toHaveBeenCalledWith({ queryKey: qk.apps.all() });
  });

  it("returns a stop function that unsubscribes", () => {
    const { unsubscribe } = makeFakeSub();
    const client = new QueryClient();
    const stop = startAppServerEventBridge(client);
    stop();
    expect(unsubscribe).toHaveBeenCalled();
  });
});
