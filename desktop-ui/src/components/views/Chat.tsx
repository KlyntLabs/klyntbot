import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { useChatSession } from "../../hooks/useChatSession";
import { ipc } from "../../hooks/useIpc";
import { useQuery } from "../../hooks/useQuery";
import { useSetToggle } from "../../hooks/useSetToggle";
import type { ChatThread } from "../../lib/types";
import { ChatInput } from "../chat/ChatInput";
import { MessageList } from "../chat/MessageList";
import { ThreadContextMenu } from "../chat/ThreadContextMenu";
import { type AreaGroup, featurePrefix, ThreadList } from "../chat/ThreadList";
import { TransparencyPanel } from "../chat/TransparencyPanel";
import { TransparencyToggle } from "../chat/TransparencyToggle";

interface GroupedThreads {
  areas: AreaGroup[];
  features: Map<string, ChatThread[]>;
  general: ChatThread[];
}

export function Chat() {
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

  // Chat session
  const chat = useChatSession(selectedThread, refetchThreads);

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
        <div className="flex items-center justify-end px-4 py-2 border-b border-white/[0.06]">
          <TransparencyToggle enabled={showTransparency} onToggle={toggleTransparency} />
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <div className="max-w-3xl mx-auto">
            {chat.messages.length === 0 && !chat.isStreaming ? (
              <div className="flex flex-col items-center justify-center py-20">
                <p className="text-muted text-sm font-light">Start a conversation</p>
                <p className="text-dim text-xs font-light mt-1">
                  Ask Klynt anything about your tasks, projects, or schedule
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
                showTransparency={showTransparency}
                liveTransparency={chat.transparency}
                activeDelegateAgent={chat.activeDelegateAgent}
              />
            )}
          </div>
        </div>

        <ChatInput
          input={chat.input}
          isStreaming={chat.isStreaming}
          onInputChange={chat.setInput}
          onSend={handleSend}
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
