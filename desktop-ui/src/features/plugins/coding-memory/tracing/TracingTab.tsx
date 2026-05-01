import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { ProviderInfo } from "./types";
import { SessionsListView } from "./SessionsListView";
import { SessionDetailView } from "./SessionDetailView";
import { StatisticsView } from "./StatisticsView";

 type RootTab = "sessions" | "statistics";

interface ViewState {
  view: "list" | "detail";
  sessionId: string | null;
}

export function TracingTab() {
  const [providerId, setProviderId] = useState<string>("kimi");
  const [rootTab, setRootTab] = useState<RootTab>("sessions");
  const [viewState, setViewState] = useState<ViewState>({ view: "list", sessionId: null });
  const [providers, setProviders] = useState<ProviderInfo[]>([]);

  useEffect(() => {
    invoke<ProviderInfo[]>("tracing_list_providers").then(setProviders).catch(() => {});
  }, []);

  return (
    <div className="tracing-root">
      <div className="tracing-providers">
        {providers.map((p) => (
          <button
            key={p.id}
            type="button"
            className={"tracing-provider" + (providerId === p.id ? " tracing-provider--active" : "")}
            onClick={() => setProviderId(p.id)}
          >{p.displayName} ({p.sessionCount})</button>
        ))}
      </div>
      <div className="tracing-tabs">
        <button
          type="button"
          className={"tracing-tab" + (rootTab === "sessions" ? " tracing-tab--active" : "")}
          onClick={() => setRootTab("sessions")}
        >
          Sessions
        </button>
        <button
          type="button"
          className={"tracing-tab" + (rootTab === "statistics" ? " tracing-tab--active" : "")}
          onClick={() => setRootTab("statistics")}
        >
          Statistics
        </button>
      </div>
      <div className="tracing-body">
        {rootTab === "sessions" ? (
          viewState.view === "list" ? (
            <SessionsListView
              providerId={providerId}
              onOpenSession={(sessionId) => setViewState({ view: "detail", sessionId })}
            />
          ) : viewState.sessionId ? (
            <SessionDetailView
              providerId={providerId}
              sessionId={viewState.sessionId}
              onBack={() => setViewState({ view: "list", sessionId: null })}
            />
          ) : null
        ) : (
          <StatisticsView providerId={providerId} />
        )}
      </div>
    </div>
  );
}
