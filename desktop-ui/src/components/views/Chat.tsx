import { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { useNavigate } from 'react-router';
import {
  Send, ChevronDown, Plus, Settings, FolderOpen, MessageSquare,
  RotateCcw, Mic, Shield, Server, Wallet, Globe,
  Pencil, Trash2, X, Check,
} from 'lucide-react';
import { Sidebar } from '../layout/Sidebar';
import { MessageList } from '../chat/MessageList';
import { useSetToggle } from '../../hooks/useSetToggle';
import { useQuery } from '../../hooks/useQuery';
import { useChatSession } from '../../hooks/useChatSession';
import { ipc } from '../../hooks/useIpc';
import type { ChatThread, SidebarItem } from '../../lib/types';

// Known feature prefixes → display config
const FEATURE_GROUPS: Record<string, { label: string; icon: typeof Wallet }> = {
  finance: { label: 'Finance', icon: Wallet },
};

function formatRelativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'now';
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  if (days < 30) return `${Math.floor(days / 7)}w`;
  return `${Math.floor(days / 30)}mo`;
}

/** Extract the feature prefix from an entity_kind like "finance.budgets" → "finance" */
function featurePrefix(entityKind?: string): string | null {
  if (!entityKind) return null;
  const dot = entityKind.indexOf('.');
  const prefix = dot > 0 ? entityKind.slice(0, dot) : entityKind;
  return FEATURE_GROUPS[prefix] ? prefix : null;
}

interface AreaGroup {
  areaId: string;
  areaName: string;
  projectGroups: Map<string, { projectName: string; threads: ChatThread[] }>;
  threads: ChatThread[]; // Area-level threads (no project)
}

interface GroupedThreads {
  areas: AreaGroup[];
  features: Map<string, ChatThread[]>;
  general: ChatThread[];
}

export function Chat() {
  const navigate = useNavigate();
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>('Chat');
  const [selectedThread, setSelectedThread] = useState(() => `chat:${crypto.randomUUID()}`);
  const [expandedGroups, toggleGroup] = useSetToggle(['_general']);

  // IPC data
  const { data: threads, refetch: refetchThreads } = useQuery<ChatThread[]>('chat_threads', undefined, []);

  // Chat session (messages, streaming, input, send)
  // Refetch thread list when the agent finishes (new thread appears in sidebar)
  const chat = useChatSession(selectedThread, refetchThreads);

  // Auto-select first thread only on initial page load.
  // Once this fires, further thread selection is user-driven.
  const didAutoSelect = useRef(false);
  useEffect(() => {
    if (didAutoSelect.current) return;
    if (threads.length > 0) {
      setSelectedThread(threads[0].sessionKey);
      didAutoSelect.current = true;
    }
  }, [threads]);

  // Group threads into PARA hierarchy, features, and general
  const grouped = useMemo<GroupedThreads>(() => {
    const areaMap = new Map<string, AreaGroup>();
    const featureMap = new Map<string, ChatThread[]>();
    const general: ChatThread[] = [];

    for (const t of threads) {
      // 1. Thread with area context → PARA hierarchy
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

      // 2. Feature thread (entity_kind matches a known feature, no area_id)
      const fp = featurePrefix(t.entityKind);
      if (fp) {
        const list = featureMap.get(fp) || [];
        list.push(t);
        featureMap.set(fp, list);
        continue;
      }

      // 3. General
      general.push(t);
    }

    return {
      areas: Array.from(areaMap.values()),
      features: featureMap,
      general,
    };
  }, [threads]);

  const handleSend = useCallback(() => {
    chat.send();
  }, [chat]);

  const handleNewThread = useCallback(() => {
    setSelectedThread(`chat:${crypto.randomUUID()}`);
  }, []);

  const selectThread = useCallback((key: string) => {
    setSelectedThread(key);
  }, []);

  // ── Thread actions (rename / delete) ─────────────────────────────────
  const [contextMenu, setContextMenu] = useState<{ thread: ChatThread; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<{ sessionKey: string; value: string } | null>(null);
  const renameRef = useRef<HTMLInputElement>(null);

  // Close context menu on outside mousedown or Escape
  useEffect(() => {
    if (!contextMenu) return;
    const closeOnClick = () => setContextMenu(null);
    const closeOnKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setContextMenu(null); };
    document.addEventListener('mousedown', closeOnClick);
    document.addEventListener('keydown', closeOnKey);
    return () => {
      document.removeEventListener('mousedown', closeOnClick);
      document.removeEventListener('keydown', closeOnKey);
    };
  }, [contextMenu]);

  // Focus rename input when it appears
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
      await ipc('chat_rename_thread', { sessionKey: renaming.sessionKey, title: renaming.value.trim() });
      refetchThreads();
    } catch { /* IPC error — thread list stays as-is */ }
    setRenaming(null);
  }, [renaming, refetchThreads]);

  const cancelRename = useCallback(() => {
    setRenaming(null);
  }, []);

  const deleteThread = useCallback(async (sessionKey: string) => {
    setContextMenu(null);
    try {
      await ipc('chat_delete_thread', { sessionKey });
      if (selectedThread === sessionKey) {
        setSelectedThread(`chat:${crypto.randomUUID()}`);
      }
      refetchThreads();
    } catch { /* IPC error — thread list stays as-is */ }
  }, [selectedThread, refetchThreads]);

  // Shared thread button renderer
  const renderThread = (thread: ChatThread) => {
    const isActive = selectedThread === thread.sessionKey;
    const isRenaming = renaming?.sessionKey === thread.sessionKey;

    if (isRenaming) {
      return (
        <div key={thread.sessionKey} className="flex items-center gap-1 px-2 py-1">
          <input
            ref={renameRef}
            value={renaming.value}
            onChange={(e) => setRenaming({ ...renaming, value: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === 'Enter') confirmRename();
              if (e.key === 'Escape') cancelRename();
            }}
            className="flex-1 min-w-0 bg-surface-highest text-primary text-[12px] font-light px-2 py-1 rounded border border-border focus:outline-none focus:border-brand"
          />
          <button onClick={confirmRename} className="text-success hover:text-success/80 shrink-0">
            <Check className="w-3.5 h-3.5" strokeWidth={2} />
          </button>
          <button onClick={cancelRename} className="text-muted hover:text-secondary shrink-0">
            <X className="w-3.5 h-3.5" strokeWidth={2} />
          </button>
        </div>
      );
    }

    return (
      <button
        key={thread.sessionKey}
        onClick={() => selectThread(thread.sessionKey)}
        onContextMenu={(e) => openContextMenu(e, thread)}
        className={`w-full flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-[12px] font-light ${
          isActive
            ? 'bg-surface-highest text-primary'
            : 'text-muted hover:bg-surface-base hover:text-secondary'
        }`}
      >
        <MessageSquare className="w-3 h-3 shrink-0" strokeWidth={1.5} />
        <span className="flex-1 text-left truncate">{thread.title}</span>
        <span className="text-[11px] shrink-0">{formatRelativeTime(thread.updatedAt)}</span>
      </button>
    );
  };

  // Collapsible group header
  const renderGroupHeader = (key: string, label: string, Icon: typeof FolderOpen) => (
    <button
      onClick={() => toggleGroup(key)}
      className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
    >
      <Icon className="w-3.5 h-3.5" strokeWidth={1.5} />
      <span className="flex-1 text-left">{label}</span>
      <ChevronDown
        className={`w-3.5 h-3.5 transition-transform ${
          expandedGroups.has(key) ? 'rotate-0' : '-rotate-90'
        }`}
        strokeWidth={1.5}
      />
    </button>
  );

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item === 'Tasks') navigate('/');
        }}
      />

      {/* Left Sidebar — Thread List */}
      <div className="w-[250px] bg-background border-r border-border flex flex-col">
        {/* Quick Links */}
        <div className="px-4 py-3 space-y-1">
          <button
            onClick={handleNewThread}
            className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary"
          >
            <Plus className="w-[13px] h-[13px]" strokeWidth={1.5} />
            New thread
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary">
            <RotateCcw className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Automations
          </button>
          <button className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-surface-base transition-colors text-[12px] font-light text-muted hover:text-secondary">
            <Settings className="w-[13px] h-[13px]" strokeWidth={1.5} />
            Skills and Apps
          </button>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-y-auto px-3 pb-3">
          <div className="space-y-3">
            {/* PARA: Area groups */}
            {grouped.areas.map((area) => (
              <div key={area.areaId}>
                {renderGroupHeader(`area:${area.areaId}`, area.areaName, FolderOpen)}
                {expandedGroups.has(`area:${area.areaId}`) && (
                  <div className="mt-1 ml-3 space-y-2">
                    {/* Project sub-groups within area */}
                    {Array.from(area.projectGroups.entries()).map(([pid, pg]) => (
                      <div key={pid}>
                        {renderGroupHeader(`proj:${pid}`, pg.projectName, FolderOpen)}
                        {expandedGroups.has(`proj:${pid}`) && (
                          <div className="mt-1 ml-3 space-y-1">
                            {pg.threads.map(renderThread)}
                          </div>
                        )}
                      </div>
                    ))}
                    {/* Area-level threads (no project) */}
                    {area.threads.length > 0 && (
                      <div className="space-y-1">
                        {area.threads.map(renderThread)}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}

            {/* Feature groups */}
            {Array.from(grouped.features.entries()).map(([prefix, fThreads]) => {
              const cfg = FEATURE_GROUPS[prefix];
              return (
                <div key={`feat:${prefix}`}>
                  {renderGroupHeader(`feat:${prefix}`, cfg.label, cfg.icon)}
                  {expandedGroups.has(`feat:${prefix}`) && (
                    <div className="mt-1 ml-3 space-y-1">
                      {fThreads.map(renderThread)}
                    </div>
                  )}
                </div>
              );
            })}

            {/* General threads */}
            {grouped.general.length > 0 && (
              <div>
                {renderGroupHeader('_general', 'General', Globe)}
                {expandedGroups.has('_general') && (
                  <div className="mt-1 ml-3 space-y-1">
                    {grouped.general.map(renderThread)}
                  </div>
                )}
              </div>
            )}

            {threads.length === 0 && (
              <div className="text-center py-8 text-muted text-[12px] font-light">
                No conversations yet
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Right-click context menu (positioned at cursor) */}
      {contextMenu && (
        <div
          onMouseDown={(e) => e.stopPropagation()}
          className="fixed z-50 bg-surface-raised border border-border rounded-lg shadow-lg py-1 min-w-[140px]"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            onClick={() => startRename(contextMenu.thread)}
            className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-secondary hover:bg-surface-base transition-colors"
          >
            <Pencil className="w-3 h-3" strokeWidth={1.5} />
            Rename
          </button>
          <button
            onClick={() => deleteThread(contextMenu.thread.sessionKey)}
            className="w-full flex items-center gap-2 px-3 py-1.5 text-[12px] font-light text-destructive hover:bg-surface-base transition-colors"
          >
            <Trash2 className="w-3 h-3" strokeWidth={1.5} />
            Delete
          </button>
        </div>
      )}

      {/* Right Panel — Conversation */}
      <div className="flex-1 flex flex-col">
        {/* Messages */}
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
              />
            )}
          </div>
        </div>

        {/* Input Area */}
        <div className="p-6">
          <div className="max-w-3xl mx-auto">
            <div className="bg-surface-base rounded-2xl flex items-center px-2 gap-2">
              <button className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0">
                <Plus className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <textarea
                value={chat.input}
                onChange={(e) => chat.setInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    handleSend();
                  }
                }}
                placeholder="Ask Klynt anything, @ to add files, / for commands"
                rows={1}
                className="flex-1 bg-transparent py-3.5 text-[13px] text-primary placeholder:text-muted focus:outline-none font-light resize-none"
                style={{ maxHeight: '200px' }}
              />
              <button className="w-8 h-8 flex items-center justify-center text-muted hover:text-secondary transition-colors shrink-0">
                <Mic className="w-4 h-4" strokeWidth={1.5} />
              </button>
              <button
                onClick={handleSend}
                disabled={!chat.input.trim() || chat.isStreaming}
                className="w-9 h-9 rounded-full bg-brand hover:bg-brand/90 disabled:bg-surface-base disabled:text-muted flex items-center justify-center transition-colors shrink-0"
              >
                <Send className="w-4 h-4" strokeWidth={2} />
              </button>
            </div>
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-2">
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                  <Server className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Local</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                  <Shield className="w-3.5 h-3.5" strokeWidth={1.5} />
                  <span>Default permissions</span>
                  <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
                </button>
              </div>
              <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-surface-base hover:bg-surface-raised transition-colors text-[11px] font-light text-muted">
                <FolderOpen className="w-3.5 h-3.5" strokeWidth={1.5} />
                <span>KlyntBot</span>
                <ChevronDown className="w-3 h-3" strokeWidth={1.5} />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
