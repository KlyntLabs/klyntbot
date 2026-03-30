import { AmbientIndicator, PromotionToast, usePromotionListener } from "@features/autotuner";
import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useSetToggle } from "@shared/hooks/useSetToggle";
import type { ChatThread, ContextResumeData } from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";
import { ChatInput } from "../components/ChatInput";
import { MessageList } from "../components/MessageList";
import { ThreadContextMenu } from "../components/ThreadContextMenu";
import { type AreaGroup, featurePrefix, ThreadList } from "../components/ThreadList";
import { useChatSession } from "../hooks/useChatSession";

interface GroupedThreads {
  areas: AreaGroup[];
  features: Map<string, ChatThread[]>;
  general: ChatThread[];
}

export function ChatPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [selectedThread, setSelectedThreadState] = useState(
    () => searchParams.get("thread") || `chat:${crypto.randomUUID()}`,
  );
  const setSelectedThread = useCallback(
    (key: string) => {
      setSelectedThreadState(key);
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.set("thread", key);
          return next;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );
  const [expandedGroups, toggleGroup] = useSetToggle(["_general"]);

  // IPC data
  const { data: threads, refetch: refetchThreads } = useQuery<ChatThread[]>(
    "chat_threads",
    undefined,
    [],
  );

  // Detect squad chat from the currently selected thread
  const currentThread = useMemo(
    () => threads.find((t) => t.sessionKey === selectedThread),
    [threads, selectedThread],
  );
  // Derive squadId from DB thread metadata, or parse from the session key pattern
  // (squad:{squadId}:{uuid}) for newly created squad chats that aren't in the DB yet.
  const squadId = useMemo(() => {
    if (currentThread?.squadId) return currentThread.squadId;
    const match = selectedThread.match(/^squad:([^:]+):/);
    return match?.[1] ?? null;
  }, [currentThread?.squadId, selectedThread]);

  // Chat session — always use debate mode for squad chats
  const chat = useChatSession(
    selectedThread,
    refetchThreads,
    squadId ? { squadId, squadMode: "debate" } : undefined,
  );

  // Context resume — pre-fill input from navigation state
  const location = useLocation();
  const resumeContext = (location.state as { resumeContext?: ContextResumeData } | null)
    ?.resumeContext;
  const [resumeBanner, setResumeBanner] = useState<string | null>(null);
  const didApplyResume = useRef(false);
  useEffect(() => {
    if (resumeContext && !didApplyResume.current) {
      didApplyResume.current = true;
      chat.setInput(resumeContext.suggestedPrompt);
      setResumeBanner(resumeContext.contextTitle);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- chat ref is unstable; didApplyResume guards double-apply
  }, [resumeContext, chat.setInput]);

  // ── Voice phase indicator ─────────────────────────────────────────────
  const [voicePhase, setVoicePhase] = useState<string>("idle");
  const voicePhaseRef = useRef(voicePhase);
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.type === "phaseChanged") {
        const next = detail.phase as string;
        if (next !== voicePhaseRef.current) {
          voicePhaseRef.current = next;
          setVoicePhase(next);
        }
      }
    };
    window.addEventListener("voice:event", handler);
    return () => window.removeEventListener("voice:event", handler);
  }, []);

  // ── Provider degradation status ───────────────────────────────────────
  const [providerStatus, setProviderStatus] = useState<"ok" | "fallback" | "offline">("ok");
  useEvent<{ level: string }>("provider:degraded", (payload) => {
    setProviderStatus(payload.level as "fallback" | "offline");
    // Auto-reset after 30s if it was just a blip
    setTimeout(() => setProviderStatus("ok"), 30_000);
  });

  // ── Autotuner promotion toast ──────────────────────────────────────────
  const [promotionImpact, setPromotionImpact] = useState<string | null>(null);
  usePromotionListener((impact) => setPromotionImpact(impact));
  useEffect(() => {
    if (!promotionImpact) return;
    const timer = setTimeout(() => setPromotionImpact(null), 15_000);
    return () => clearTimeout(timer);
  }, [promotionImpact]);

  // Auto-select first thread on initial load (skip if URL already has a thread)
  const didAutoSelect = useRef(!!searchParams.get("thread"));
  useEffect(() => {
    if (didAutoSelect.current) return;
    if (threads.length > 0) {
      setSelectedThread(threads[0].sessionKey);
      didAutoSelect.current = true;
    }
  }, [threads, setSelectedThread]);

  // Group threads into PARA hierarchy, features, and general
  const grouped = useMemo<GroupedThreads>(() => {
    const areaMap = new Map<string, AreaGroup>();
    const featureMap = new Map<string, ChatThread[]>();
    const general: ChatThread[] = [];

    for (const t of threads) {
      if (t.areaId) {
        let area = areaMap.get(t.areaId);
        if (!area) {
          area = {
            areaId: t.areaId,
            areaName: t.areaName || t.areaId,
            projectGroups: new Map(),
            threads: [],
          };
          areaMap.set(t.areaId, area);
        }
        if (t.projectId) {
          let pg = area.projectGroups.get(t.projectId);
          if (!pg) {
            pg = { projectName: t.projectName || t.projectId, threads: [] };
            area.projectGroups.set(t.projectId, pg);
          }
          pg.threads.push(t);
        } else {
          area.threads.push(t);
        }
        continue;
      }
      const fp = featurePrefix(t.entityKind);
      if (fp) {
        const list = featureMap.get(fp) || [];
        list.push(t);
        featureMap.set(fp, list);
        continue;
      }
      general.push(t);
    }

    return { areas: Array.from(areaMap.values()), features: featureMap, general };
  }, [threads]);

  const handleSend = () => {
    chat.send();
  };

  const handleNewThread = () => {
    setSelectedThread(`chat:${crypto.randomUUID()}`);
  };

  const handleNewSquadThread = useCallback(
    (newSquadId: string) => {
      setSelectedThread(`squad:${newSquadId}:${crypto.randomUUID()}`);
    },
    [setSelectedThread],
  );

  // ── Thread actions ─────────────────────────────────────────────────────
  const [contextMenu, setContextMenu] = useState<{
    thread: ChatThread;
    x: number;
    y: number;
  } | null>(null);
  const [renaming, setRenaming] = useState<{ sessionKey: string; value: string } | null>(null);
  const [confirmDeleteThread, setConfirmDeleteThread] = useState<string | null>(null);
  const renameRef = useRef<HTMLInputElement>(null);

  // Close context menu on outside mousedown or Escape
  useEffect(() => {
    if (!contextMenu) {
      setConfirmDeleteThread(null);
      return;
    }
    const closeOnClick = () => setContextMenu(null);
    const closeOnKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setContextMenu(null);
    };
    document.addEventListener("mousedown", closeOnClick);
    document.addEventListener("keydown", closeOnKey);
    return () => {
      document.removeEventListener("mousedown", closeOnClick);
      document.removeEventListener("keydown", closeOnKey);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (renaming) renameRef.current?.focus();
  }, [renaming]);

  const openContextMenu = useCallback((e: React.MouseEvent, thread: ChatThread) => {
    e.preventDefault();
    setContextMenu({ thread, x: e.clientX, y: e.clientY });
  }, []);

  const startRename = useCallback((thread: ChatThread) => {
    setContextMenu(null);
    setRenaming({ sessionKey: thread.sessionKey, value: thread.title });
  }, []);

  const confirmRename = useCallback(async () => {
    if (!renaming || !renaming.value.trim()) return;
    try {
      await ipc("chat_rename_thread", {
        sessionKey: renaming.sessionKey,
        title: renaming.value.trim(),
      });
      refetchThreads();
    } catch {}
    setRenaming(null);
  }, [renaming, refetchThreads]);

  const cancelRename = useCallback(() => {
    setRenaming(null);
  }, []);

  const deleteThread = useCallback(
    async (sessionKey: string) => {
      if (confirmDeleteThread !== sessionKey) {
        setConfirmDeleteThread(sessionKey);
        return;
      }
      setConfirmDeleteThread(null);
      setContextMenu(null);
      try {
        await ipc("chat_delete_thread", { sessionKey });
        if (selectedThread === sessionKey) {
          setSelectedThread(`chat:${crypto.randomUUID()}`);
        }
        refetchThreads();
      } catch {}
    },
    [selectedThread, refetchThreads, confirmDeleteThread, setSelectedThread],
  );

  return (
    <>
      <ThreadList
        threads={threads}
        grouped={grouped}
        selectedThread={selectedThread}
        expandedGroups={expandedGroups}
        renaming={renaming}
        renameRef={renameRef}
        onSelectThread={setSelectedThread}
        onNewThread={handleNewThread}
        onToggleGroup={toggleGroup}
        onContextMenu={openContextMenu}
        onRenameChange={(value) => setRenaming((r) => (r ? { ...r, value } : null))}
        onRenameConfirm={confirmRename}
        onRenameCancel={cancelRename}
      />

      {contextMenu && (
        <ThreadContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          thread={contextMenu.thread}
          onRename={startRename}
          onDelete={deleteThread}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* Right Panel — Conversation */}
      <div className="flex-1 flex flex-col overflow-hidden rounded-xl relative">
        {/* Toolbar: resume banner, squad header, voice toggle, transparency */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-border-subtle">
          {resumeBanner ? (
            <div className="flex items-center gap-2 text-xs">
              <span className="text-brand font-medium">Resuming: {resumeBanner}</span>
              <button
                type="button"
                onClick={() => setResumeBanner(null)}
                className="text-muted-foreground hover:text-foreground"
              >
                ×
              </button>
            </div>
          ) : (
            <div />
          )}
          <div className="flex items-center gap-2">
            {voicePhase !== "idle" && (
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
                <span className="h-2 w-2 rounded-full bg-success animate-pulse" />
                <span>
                  {voicePhase === "listening"
                    ? "Listening"
                    : voicePhase === "reflecting"
                      ? "Reflecting"
                      : "Speaking"}
                </span>
              </div>
            )}
            <AmbientIndicator onClick={() => navigate("/settings/general")} />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-3xl mx-auto">
            {chat.messages.length === 0 && !chat.isStreaming ? (
              <div className="flex flex-col items-center justify-center py-20">
                <p className="text-muted-foreground text-sm font-light">
                  {squadId ? "Start a squad conversation" : "Start a conversation"}
                </p>
                <p className="text-dim text-xs font-light mt-1">
                  {squadId
                    ? "Your squad members will each share their perspective"
                    : "Ask Klynt anything about your tasks, projects, or schedule"}
                </p>
              </div>
            ) : (
              <MessageList
                messages={chat.messages}
                segments={chat.segments}
                isStreaming={chat.isStreaming}
                activeTools={chat.activeTools}
                error={chat.error}
                activeInteraction={chat.activeInteraction}
                sessionKey={selectedThread}
                onInteractionSubmitted={() => {
                  chat.clearInteraction();
                  refetchThreads();
                }}
                liveTransparency={chat.transparency}
                activeDelegateAgent={chat.activeDelegateAgent}
                statusPhase={chat.statusPhase}
                personaMessages={
                  squadId && chat.personaMessages.length > 0 ? chat.personaMessages : undefined
                }
              />
            )}
          </div>
        </div>

        {promotionImpact && (
          <div className="px-6 pb-2">
            <div className="max-w-3xl mx-auto">
              <PromotionToast impact={promotionImpact} onDismiss={() => setPromotionImpact(null)} />
            </div>
          </div>
        )}

        {providerStatus === "fallback" && (
          <div className="px-6 pb-2">
            <div className="max-w-3xl mx-auto">
              <div className="px-4 py-2 text-xs text-amber-400 bg-amber-400/5 rounded-lg">
                Claude is taking a moment. I'm working from what I already know about you — give me
                a sec.
              </div>
            </div>
          </div>
        )}
        {providerStatus === "offline" && (
          <div className="px-6 pb-2">
            <div className="max-w-3xl mx-auto">
              <div className="px-4 py-2 text-xs text-muted-foreground bg-accent rounded-lg">
                All my cloud connections are down right now. I can still search your tasks, notes,
                and memory locally — just ask.
              </div>
            </div>
          </div>
        )}

        <ChatInput
          input={chat.input}
          isStreaming={chat.isStreaming}
          squadId={squadId}
          onInputChange={chat.setInput}
          onSend={handleSend}
          onSelectSquad={handleNewSquadThread}
          onSelectDefault={handleNewThread}
        />
      </div>
    </>
  );
}
