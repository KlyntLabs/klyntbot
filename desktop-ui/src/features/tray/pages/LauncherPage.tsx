import { KlyntLogo } from "@shared/components/ui/KlyntLogo";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { isTauri } from "@shared/lib/utils";
import type { LauncherItem } from "@shared/types";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Calendar,
  Command,
  MessageSquare,
  Plus,
  Search,
  Settings,
  Sparkles,
  Target,
  TrendingUp,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { LauncherChat } from "../components/LauncherChat";

const launcherItems: LauncherItem[] = [
  {
    id: "1",
    title: "Create New Task",
    subtitle: "Quick add a new task",
    icon: "Plus",
    shortcut: "\u2318N",
  },
  {
    id: "2",
    title: "Search Tasks",
    subtitle: "Find tasks across all projects",
    icon: "Search",
    shortcut: "\u2318K",
  },
  {
    id: "3",
    title: "Open Chat",
    subtitle: "Talk to AI assistant",
    icon: "MessageSquare",
    shortcut: "\u2318/",
  },
  {
    id: "4",
    title: "View Calendar",
    subtitle: "See upcoming events",
    icon: "Calendar",
    shortcut: "\u2318C",
  },
  {
    id: "5",
    title: "Today's Focus",
    subtitle: "View daily priorities",
    icon: "Target",
    shortcut: "\u2318T",
  },
  {
    id: "6",
    title: "Review OKRs",
    subtitle: "Check goal progress",
    icon: "TrendingUp",
    shortcut: "\u2318O",
  },
  {
    id: "7",
    title: "Settings",
    subtitle: "App preferences",
    icon: "Settings",
    shortcut: "\u2318,",
  },
];

const iconMap: Record<string, typeof Search> = {
  Plus,
  Search,
  MessageSquare,
  Calendar,
  Target,
  TrendingUp,
  Settings,
  Sparkles,
};

export function Launcher() {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [mode, setMode] = useState<"command" | "chat">("command");
  const [sessionKey, setSessionKey] = useState<string | null>(null);
  const [initialQuery, setInitialQuery] = useState<string | null>(null);

  useTransparentBackground({ nativeVibrancy: true });

  const enterChat = useCallback((text: string) => {
    const key = `launcher-${Date.now()}`;
    setSessionKey(key);
    setInitialQuery(text);
    setMode("chat");
    setQuery("");
  }, []);

  const exitChat = useCallback(() => {
    setMode("command");
    setSessionKey(null);
    setInitialQuery(null);
  }, []);

  const expandToMain = useCallback(async () => {
    if (!sessionKey || !isTauri) return;
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const [mainWindow] = await Promise.all([
      WebviewWindow.getByLabel("main"),
      emit("open-chat", { sessionKey }),
    ]);
    if (mainWindow) {
      await mainWindow.show();
      await mainWindow.setFocus();
    }
    await getCurrentWindow().hide();
    exitChat();
  }, [sessionKey, exitChat]);

  const filteredItems = useMemo(() => {
    const q = query.toLowerCase();
    return launcherItems.filter(
      (item) => item.title.toLowerCase().includes(q) || item.subtitle.toLowerCase().includes(q),
    );
  }, [query]);

  // Escape key: mode-aware
  useEffect(() => {
    const handleGlobalKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (mode === "chat") {
          exitChat();
        } else if (isTauri) {
          getCurrentWindow().hide();
        }
      }
    };
    window.addEventListener("keydown", handleGlobalKeyDown);
    return () => window.removeEventListener("keydown", handleGlobalKeyDown);
  }, [mode, exitChat]);

  const contentRef = useRef<HTMLDivElement>(null);
  useWindowAutoResize(contentRef, { width: 660, maxHeight: 580 });

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, filteredItems.length - (query.trim() ? 0 : 1)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (query.trim() && selectedIndex === 0) {
        enterChat(query.trim());
      }
    } else if (e.key === "Tab" && query.trim()) {
      e.preventDefault();
      enterChat(query.trim());
    }
  };

  return (
    <div className="w-screen text-primary">
      <div
        ref={contentRef}
        className="w-full glass-floating overflow-hidden"
        style={{ animation: "glass-appear 0.25s ease-out" }}
      >
        {mode === "command" ? (
          <div className="rounded-[var(--glass-radius-inner)] overflow-hidden">
            {/* Header */}
            <div className="px-5 pt-5 pb-3">
              <div className="flex items-center gap-2.5 mb-1">
                <div className="w-7 h-7 rounded-lg bg-white/90 flex items-center justify-center p-1 ">
                  <KlyntLogo className="w-full h-full" />
                </div>
                <h1 className="text-[15px] font-medium text-primary tracking-tight">
                  Klynt Launcher
                </h1>
              </div>
              <p className="text-[11px] text-muted font-light">AI assistant & quick actions</p>
            </div>

            {/* Search Bar */}
            <div className="px-5 pb-4">
              <div className="glass-input flex items-center gap-3 px-4 py-3">
                <Sparkles className="w-[18px] h-[18px] text-brand" strokeWidth={1.5} />
                <input
                  type="text"
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setSelectedIndex(0);
                  }}
                  onKeyDown={handleKeyDown}
                  placeholder="Ask Klynt anything or type a command\u2026"
                  aria-label="Search commands"
                  className="flex-1 bg-transparent text-primary text-[13px] placeholder:text-muted outline-none font-light"
                />
                <div className="flex items-center gap-1.5">
                  <span className="glass-badge px-2 py-0.5 text-[10px] text-muted font-light">
                    AI
                  </span>
                  <span className="glass-badge px-2 py-0.5 text-[10px] text-muted font-light">
                    Tab
                  </span>
                </div>
              </div>
            </div>

            {/* Results */}
            <div className="max-h-[320px] overflow-y-auto px-5 pb-4">
              {query.trim() || filteredItems.length > 0 ? (
                <div className="space-y-1">
                  {/* AI Query Option */}
                  {query.trim() && (
                    <button
                      type="button"
                      className={`w-full flex items-center gap-4 px-4 py-3 rounded-xl transition-all duration-150 ${
                        0 === selectedIndex
                          ? "bg-white/[0.08] border border-white/[0.1]"
                          : "border border-transparent hover:bg-white/[0.04]"
                      }`}
                      onMouseEnter={() => setSelectedIndex(0)}
                      onClick={() => enterChat(query.trim())}
                    >
                      <div
                        className={`w-9 h-9 rounded-xl flex items-center justify-center transition-all duration-150 ${
                          0 === selectedIndex
                            ? "bg-brand/90 text-white"
                            : "bg-white/[0.06] text-brand"
                        }`}
                      >
                        <Sparkles className="w-[18px] h-[18px]" strokeWidth={1.5} />
                      </div>
                      <div className="flex-1 text-left">
                        <h3 className="text-primary text-[13px] font-light">
                          Ask Klynt AI: {query}
                        </h3>
                        <p className="text-[11px] text-muted font-light">Get AI-powered response</p>
                      </div>
                    </button>
                  )}

                  {/* Commands */}
                  {filteredItems.map((item, index) => {
                    const Icon = iconMap[item.icon] ?? Search;
                    const actualIndex = query.trim() ? index + 1 : index;
                    const isSelected = actualIndex === selectedIndex;
                    return (
                      <button
                        type="button"
                        key={item.id}
                        className={`w-full flex items-center gap-4 px-4 py-3 rounded-xl transition-all duration-150 ${
                          isSelected
                            ? "bg-black/[0.06] border border-black/[0.06]"
                            : "border border-transparent hover:bg-black/[0.03]"
                        }`}
                        onMouseEnter={() => setSelectedIndex(actualIndex)}
                      >
                        <div
                          className={`w-9 h-9 rounded-xl flex items-center justify-center transition-all duration-150 ${
                            isSelected ? "bg-brand/90 text-white" : "bg-white/[0.06] text-muted"
                          }`}
                          style={
                            isSelected ? { boxShadow: "0 4px 20px var(--brand-glow)" } : undefined
                          }
                        >
                          <Icon
                            className="w-[18px] h-[18px]"
                            strokeWidth={1.5}
                            aria-hidden="true"
                          />
                        </div>
                        <div className="flex-1 text-left">
                          <h3 className="text-primary text-[13px] font-light">{item.title}</h3>
                          <p className="text-[11px] text-muted font-light">{item.subtitle}</p>
                        </div>
                        <div className="flex items-center gap-1 text-[11px] text-dim font-light">
                          {item.shortcut}
                        </div>
                      </button>
                    );
                  })}
                </div>
              ) : (
                <div className="px-5 py-10 text-center text-muted text-[13px] font-light">
                  No results found
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="px-5 py-3">
              <div className="glass-divider mb-3" />
              <div className="flex items-center justify-between text-[11px] text-dim">
                <div className="flex items-center gap-4">
                  <span className="flex items-center gap-1.5 font-light">
                    <kbd className="glass-badge px-1.5 py-0.5 text-[10px]">
                      <span className="text-muted">{"\u21B5"}</span>
                    </kbd>
                    Open
                  </span>
                  <span className="flex items-center gap-1.5 font-light">
                    <kbd className="glass-badge px-1.5 py-0.5 text-[10px] text-muted">Esc</kbd>
                    Close
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="flex items-center gap-1.5 font-light">
                    <Command className="w-3 h-3 text-muted" strokeWidth={1.5} />
                    Commands
                  </span>
                </div>
              </div>
            </div>
          </div>
        ) : sessionKey ? (
          <LauncherChat
            sessionKey={sessionKey}
            initialQuery={initialQuery}
            onBack={exitChat}
            onExpand={expandToMain}
          />
        ) : null}
      </div>
    </div>
  );
}
