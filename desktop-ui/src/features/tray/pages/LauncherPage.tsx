import { ActionMenu } from "@features/launcher/components/ActionMenu";
import { ArgChipBar } from "@features/launcher/components/ArgChipBar";
import { DetailPanel } from "@features/launcher/components/DetailPanel";
import { FocusActiveChip } from "@features/launcher/components/FocusActiveChip";
import { LauncherChat } from "@features/launcher/components/LauncherChat";
import { LauncherInput } from "@features/launcher/components/LauncherInput";
import { ResultsList } from "@features/launcher/components/ResultsList";
import { VoiceRecorder } from "@features/launcher/components/VoiceRecorder";
import { useDashboardData } from "@features/launcher/hooks/useDashboardData";
import { useDndActive } from "@features/launcher/hooks/useDndActive";
import { executeItem } from "@features/launcher/hooks/useExecuteItem";
import { useKeyboardNavigation } from "@features/launcher/hooks/useKeyboardNavigation";
import { useLauncherSearch } from "@features/launcher/hooks/useLauncherSearch";
import { useLauncherStore } from "@features/launcher/stores/launcherStore";
import { useFocusOnboarding } from "@features/settings/components/FocusOnboarding";
import { ipc } from "@shared/hooks/useIpc";
import { useTransparentBackground } from "@shared/hooks/useTransparentBackground";
import { useWindowAutoResize } from "@shared/hooks/useWindowAutoResize";
import { isTauri } from "@shared/lib/utils";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";

export function Launcher() {
  const contentRef = useRef<HTMLDivElement>(null);
  const mode = useLauncherStore((s) => s.mode);
  const setMode = useLauncherStore((s) => s.setMode);
  const reset = useLauncherStore((s) => s.reset);
  const argModeItem = useLauncherStore((s) => s.argModeItem);
  const setArgModeItem = useLauncherStore((s) => s.setArgModeItem);
  const dndActive = useDndActive();
  const focusOnboarding = useFocusOnboarding();

  const [chatSessionKey, setChatSessionKey] = useState("");
  const [chatInitialQuery, setChatInitialQuery] = useState<string | null>(null);

  useTransparentBackground({ nativeVibrancy: true });
  useWindowAutoResize(contentRef, { width: 660, maxHeight: 680 });
  useLauncherSearch();
  useDashboardData();

  const hideWindow = useCallback(async () => {
    if (isTauri) {
      try {
        await getCurrentWindow().hide();
      } catch {
        // Window hide failed silently
      }
    }
    reset();
  }, [reset]);

  const enterChat = useCallback(
    (query: string) => {
      setChatSessionKey(`launcher-${Date.now()}`);
      setChatInitialQuery(query);
      setMode("chat");
    },
    [setMode],
  );

  const handleTranscriptReady = useCallback(
    (transcript: string) => {
      enterChat(transcript);
    },
    [enterChat],
  );

  const cancelRecording = useCallback(async () => {
    reset();
    if (isTauri) {
      try {
        await getCurrentWindow().hide();
      } catch {
        // Window hide failed silently
      }
    }
  }, [reset]);

  const navigateToMain = useCallback(
    async (path: string, event?: { name: string; payload: unknown }) => {
      if (!isTauri) return;
      try {
        const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const mainWindow = await WebviewWindow.getByLabel("main");
        if (mainWindow) {
          if (event) {
            await emit(event.name, event.payload);
          }
          await emit("navigate", { path });
          await mainWindow.show();
          await mainWindow.setFocus();
        }
        await getCurrentWindow().hide();
      } catch {
        // Tauri API unavailable
      }
      reset();
    },
    [reset],
  );

  const expandToMain = useCallback(async () => {
    await navigateToMain(
      chatSessionKey ? "/chat" : "/",
      chatSessionKey ? { name: "open-chat", payload: { sessionKey: chatSessionKey } } : undefined,
    );
  }, [chatSessionKey, navigateToMain]);

  useEffect(() => {
    if (!isTauri) return;
    const unlisteners: (() => void)[] = [];

    listen("voice-recording-start", () => {
      // Reset any previous state before starting fresh recording
      reset();
      setMode("recording");
    }).then((fn) => unlisteners.push(fn));

    listen("voice-recording-reset", () => {
      // Cancel any active capture before resetting
      ipc("voice_dismiss", {}).catch(() => {});
      reset();
    }).then((fn) => unlisteners.push(fn));

    return () => {
      for (const fn of unlisteners) fn();
    };
  }, [setMode, reset]);

  useKeyboardNavigation({
    onEnterChat: enterChat,
    onExpandToMain: expandToMain,
    onHide: hideWindow,
  });

  const handleExecute = useCallback(
    (index: number) => {
      const results = useLauncherStore.getState().results;
      const item = results[index];
      if (!item) return;

      // When the user invokes the DND item while a session is already active,
      // show the extend/off chip bar instead of launching the arg chip.
      if (
        item.kind.type === "systemCommand" &&
        item.kind.action === "toggleDoNotDisturb" &&
        dndActive.data
      ) {
        setArgModeItem(item);
        return;
      }

      executeItem(item, {
        onEnterChat: enterChat,
        onExpandToMain: expandToMain,
        onHide: hideWindow,
        onNeedsOnboarding: (retryFn) => focusOnboarding.open(retryFn),
      });
    },
    [enterChat, expandToMain, hideWindow, dndActive.data, setArgModeItem, focusOnboarding],
  );

  return (
    <div className="w-screen text-foreground">
      {focusOnboarding.element}
      <div
        ref={contentRef}
        className="w-full glass-floating overflow-hidden"
        style={{ animation: "glass-appear 0.25s ease-out" }}
      >
        {/* Draggable handle — lets user reposition the launcher window */}
        <div
          onMouseDown={async () => {
            if (!isTauri) return;
            try {
              await getCurrentWindow().startDragging();
            } catch {
              // Drag not supported or window not ready
            }
          }}
          className="h-3 w-full cursor-grab active:cursor-grabbing flex items-center justify-center"
        >
          <div className="w-8 h-[3px] rounded-full bg-border-subtle/40" />
        </div>
        {mode === "recording" ? (
          <VoiceRecorder onTranscriptReady={handleTranscriptReady} onCancel={cancelRecording} />
        ) : mode === "chat" && chatSessionKey ? (
          <LauncherChat
            sessionKey={chatSessionKey}
            initialQuery={chatInitialQuery}
            onBack={() => {
              setMode("dashboard");
              reset();
            }}
            onExpand={expandToMain}
          />
        ) : (
          <div className="relative rounded-[var(--glass-radius-inner)] overflow-hidden">
            <LauncherInput />
            {argModeItem &&
            argModeItem.kind.type === "systemCommand" &&
            argModeItem.kind.action === "toggleDoNotDisturb" &&
            dndActive.data ? (
              <FocusActiveChip endsAt={dndActive.data.endsAt} onDone={() => setArgModeItem(null)} />
            ) : argModeItem ? (
              <ArgChipBar
                specs={argModeItem.arguments ?? []}
                onSubmit={(args) => {
                  setArgModeItem(null);
                  executeItem(
                    argModeItem,
                    {
                      onEnterChat: enterChat,
                      onExpandToMain: expandToMain,
                      onHide: hideWindow,
                      onNeedsOnboarding: (retryFn) => focusOnboarding.open(retryFn),
                    },
                    args,
                  );
                }}
                onCancel={() => setArgModeItem(null)}
              />
            ) : (
              <>
                {mode === "dashboard" && <ShortcutHints />}
                {mode === "search" && <ResultsList onExecute={handleExecute} />}
                {mode === "detail" && <DetailPanel />}
              </>
            )}
            <ActionMenu />
          </div>
        )}
      </div>
    </div>
  );
}

function ShortcutHints() {
  const hints = [
    { key: "f/", label: "Files" },
    { key: "g/", label: "Grep" },
    { key: "h/", label: "History" },
    { key: "@", label: "Contacts" },
    { key: ">", label: "Commands" },
    { key: "?", label: "Ask AI" },
  ];

  return (
    <div className="px-3 py-2.5 flex items-center gap-2 flex-wrap">
      {hints.map((h) => (
        <span
          key={h.key}
          className="inline-flex items-center gap-1 text-[11px] text-muted-foreground"
        >
          <kbd className="px-1.5 py-0.5 rounded bg-accent/60 text-foreground/70 font-mono text-[10px]">
            {h.key}
          </kbd>
          {h.label}
        </span>
      ))}
    </div>
  );
}
