import { ActionMenu } from "@features/launcher/components/ActionMenu";
import { Dashboard } from "@features/launcher/components/Dashboard";
import { DetailPanel } from "@features/launcher/components/DetailPanel";
import { LauncherChat } from "@features/launcher/components/LauncherChat";
import { LauncherInput } from "@features/launcher/components/LauncherInput";
import { ResultsList } from "@features/launcher/components/ResultsList";
import { VoiceRecorder } from "@features/launcher/components/VoiceRecorder";
import { useDashboardData } from "@features/launcher/hooks/useDashboardData";
import { executeItem } from "@features/launcher/hooks/useExecuteItem";
import { useKeyboardNavigation } from "@features/launcher/hooks/useKeyboardNavigation";
import { useLauncherSearch } from "@features/launcher/hooks/useLauncherSearch";
import { useLauncherStore } from "@features/launcher/stores/launcherStore";
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

  const cancelRecording = useCallback(() => {
    setMode("dashboard");
  }, [setMode]);

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
      executeItem(item, {
        onEnterChat: enterChat,
        onExpandToMain: expandToMain,
        onHide: hideWindow,
      });
    },
    [enterChat, expandToMain, hideWindow],
  );

  return (
    <div className="w-screen text-foreground">
      <div
        ref={contentRef}
        className="w-full glass-floating overflow-hidden"
        style={{ animation: "glass-appear 0.25s ease-out" }}
      >
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
            {mode === "dashboard" && (
              <Dashboard onOpenTask={(id) => navigateToMain(`/task/${id}`)} />
            )}
            {mode === "search" && <ResultsList onExecute={handleExecute} />}
            {mode === "detail" && <DetailPanel />}
            <ActionMenu />
          </div>
        )}
      </div>
    </div>
  );
}
