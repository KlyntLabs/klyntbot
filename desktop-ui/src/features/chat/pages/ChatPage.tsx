import { ipc } from "@shared/hooks/useIpc";
import { useQuery } from "@shared/hooks/useQuery";
import { useSetToggle } from "@shared/hooks/useSetToggle";
import type { ChatThread, ContextResumeData } from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useSearchParams } from "react-router";
import { ChatInput } from "../components/ChatInput";
import { CoachingNudge } from "../components/CoachingNudge";
import { DebateView } from "../components/DebateView";
import { MessageList } from "../components/MessageList";
import { ThreadContextMenu } from "../components/ThreadContextMenu";
import { type AreaGroup, featurePrefix, ThreadList } from "../components/ThreadList";
import { TransparencyPanel } from "../components/TransparencyPanel";
import { TransparencyToggle } from "../components/TransparencyToggle";
import type { VoiceMode } from "../components/VoiceToggle";
import { useChatSession } from "../hooks/useChatSession";

interface GroupedThreads {
  areas: AreaGroup[];
  features: Map<string, ChatThread[]>;
  general: ChatThread[];
}

export function ChatPage() {
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

  // Voice mode for squad chat
  const [voiceMode, setVoiceMode] = useState<VoiceMode>("multi");

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
  }, [resumeContext]);

  const [showTransparency, setShowTransparency] = useState(() => {
    try {
      return localStorage.getItem("chat:transparency") === "true";
    } catch {
      return false;
    }
  });
  const toggleTransparency = useCallback(() => {
    setShowTransparency((prev) => {
      const next = !prev;
      try {
        localStorage.setItem("chat:transparency", String(next));
      } catch {}
      return next;
    });
  }, []);

  const lastAssistantTransparency = useMemo(() => {
    for (let i = chat.messages.length - 1; i >= 0; i--) {
      const m = chat.messages[i];
      if (m.role === "assistant" && m.transparency) return m.transparency;
    }
    return null;
  }, [chat.messages]);
  const activeTransparency =
    chat.isStreaming && chat.transparency ? chat.transparency : lastAssistantTransparency;

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

  // Whether to show persona messages instead of the normal assistant bubble
  const showPersonaMessages = squadId && voiceMode === "multi" && chat.personaMessages.length > 0;

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
            <div className="flex items-center gap-2 text-[12px]">
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
            <TransparencyToggle enabled={showTransparency} onToggle={toggleTransparency} />
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
              <>
                {chat.debateRounds.length > 0 && (
                  <div className="mb-6">
                    <DebateView
                      rounds={chat.debateRounds}
                      totalRounds={chat.totalDebateRounds ?? 6}
                      currentRound={chat.debateRounds.at(-1)?.round ?? null}
                      consensusReached={chat.consensusReached}
                      consensusSummary={chat.consensusSummary}
                      judgeDecisions={chat.judgeDecisions}
                    />
                  </div>
                )}
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
                  showTransparency={showTransparency}
                  liveTransparency={chat.transparency}
                  activeDelegateAgent={chat.activeDelegateAgent}
                  personaMessages={showPersonaMessages ? chat.personaMessages : undefined}
                />
              </>
            )}
          </div>
        </div>

        <CoachingNudge isStreaming={chat.isStreaming} />

        <ChatInput
          input={chat.input}
          isStreaming={chat.isStreaming}
          squadId={squadId}
          voiceMode={voiceMode}
          onInputChange={chat.setInput}
          onSend={handleSend}
          onSelectSquad={handleNewSquadThread}
          onSelectDefault={handleNewThread}
          onVoiceModeChange={setVoiceMode}
        />

        {/* Floating transparency overlay */}
        {showTransparency && activeTransparency && (
          <div
            className="absolute top-12 right-3 z-30"
            style={{ animation: "fade-in 0.15s ease-out" }}
          >
            <TransparencyPanel transparency={activeTransparency} />
          </div>
        )}
      </div>
    </>
  );
}
