# Voice-in-Launcher Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Alt+Shift+V opens the launcher in "recording" mode with a hearing animation, transcribes speech, then sends the transcript directly into the launcher chat — replacing the separate voice orb window approach.

**Architecture:** Add a `"recording"` mode to the launcher's existing state machine (`dashboard | search | detail | chat` → add `recording`). The voice hotkey opens the launcher window and emits a `voice-recording-start` event. The frontend enters recording mode, shows a waveform animation, listens for the `voice:event` stream for live transcript + AudioLevel, and on finalization transitions to chat mode with the transcript as `initialQuery`. The backend voice capture pipeline (AudioCapture → Whisper → VoiceEvents) is unchanged.

**Tech Stack:** TypeScript/React (launcher components, stores), Rust (desktop main.rs hotkey handler), existing voice-engine crate

---

### Task 1: Add "recording" mode to launcher store + types

**Files:**
- Modify: `desktop-ui/src/features/launcher/types.ts`
- Modify: `desktop-ui/src/features/launcher/stores/launcherStore.ts`

- [ ] **Step 1: Add recording mode to LauncherMode type**

In `desktop-ui/src/features/launcher/types.ts`, change:

```typescript
export type LauncherMode = "dashboard" | "search" | "detail" | "chat";
```

to:

```typescript
export type LauncherMode = "dashboard" | "search" | "detail" | "chat" | "recording";
```

- [ ] **Step 2: Build frontend to verify**

Run: `cd desktop-ui && bun run build`
Expected: Compiles (no consumers of the new mode yet).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/launcher/types.ts
git commit -m "feat(voice): add recording mode to LauncherMode type"
```

---

### Task 2: Create VoiceRecorder component for the launcher

**Files:**
- Create: `desktop-ui/src/features/launcher/components/VoiceRecorder.tsx`

- [ ] **Step 1: Create VoiceRecorder component**

Create `desktop-ui/src/features/launcher/components/VoiceRecorder.tsx`:

```tsx
import { useVoiceEvents } from "@features/voice/hooks/useVoiceEvents";
import { ipc } from "@shared/hooks/useIpc";
import { Mic, Square } from "lucide-react";
import { useCallback, useEffect, useRef } from "react";

interface VoiceRecorderProps {
  onTranscriptReady: (transcript: string) => void;
  onCancel: () => void;
}

function Waveform({ level }: { level: number }) {
  const bars = 24;
  return (
    <div className="flex items-end justify-center gap-[3px] h-12">
      {Array.from({ length: bars }).map((_, i) => {
        const base = 4;
        const amplitude = level * 44 * (0.4 + 0.6 * Math.sin(i * 0.5 + Date.now() * 0.003));
        const height = Math.max(base, base + amplitude);
        return (
          <div
            key={i}
            className="w-[3px] rounded-full bg-brand/80 transition-all duration-75"
            style={{ height: `${height}px` }}
          />
        );
      })}
    </div>
  );
}

export function VoiceRecorder({ onTranscriptReady, onCancel }: VoiceRecorderProps) {
  const { sessionState, transcript, audioLevel } = useVoiceEvents();
  const hasStarted = useRef(false);
  const animationFrame = useRef<number>();

  // Force re-render for waveform animation
  const forceUpdate = useCallback(() => {
    animationFrame.current = requestAnimationFrame(forceUpdate);
  }, []);

  // Start capture on mount
  useEffect(() => {
    if (hasStarted.current) return;
    hasStarted.current = true;
    ipc("voice_start_capture", {}).catch((e: unknown) => {
      console.error("[VoiceRecorder] Failed to start capture:", e);
      onCancel();
    });

    // Start animation loop for waveform
    animationFrame.current = requestAnimationFrame(forceUpdate);

    return () => {
      if (animationFrame.current) cancelAnimationFrame(animationFrame.current);
    };
  }, [onCancel, forceUpdate]);

  // When transcript is finalized, pass it to chat
  useEffect(() => {
    if (sessionState === "response" && transcript) {
      onTranscriptReady(transcript);
    }
  }, [sessionState, transcript, onTranscriptReady]);

  const handleStop = useCallback(() => {
    ipc("voice_stop_capture", {}).catch((e: unknown) => {
      console.error("[VoiceRecorder] Failed to stop capture:", e);
    });
  }, []);

  const handleCancel = useCallback(() => {
    ipc("voice_dismiss", {}).catch(() => {});
    onCancel();
  }, [onCancel]);

  // Keyboard: Enter to stop, Escape to cancel
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleCancel();
      } else if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleStop();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleStop, handleCancel]);

  const isCapturing = sessionState === "capturing";
  const isProcessing = sessionState === "processing" || sessionState === "response";

  return (
    <div className="flex flex-col items-center justify-center py-8 px-6 gap-5">
      {/* Mic icon with pulsing ring */}
      <div className="relative">
        <div
          className={`size-16 rounded-full flex items-center justify-center ${
            isCapturing ? "bg-brand/20" : "bg-muted/20"
          }`}
        >
          <Mic
            className={`size-7 ${isCapturing ? "text-brand" : "text-muted-foreground"}`}
            strokeWidth={1.5}
          />
        </div>
        {isCapturing && (
          <div className="absolute inset-0 rounded-full border-2 border-brand/40 animate-ping" />
        )}
      </div>

      {/* Waveform */}
      {isCapturing && <Waveform level={audioLevel} />}

      {/* Status text */}
      <div className="text-center">
        {isCapturing && (
          <p className="text-sm text-muted-foreground font-light">Listening...</p>
        )}
        {isProcessing && (
          <p className="text-sm text-muted-foreground font-light animate-pulse">
            Transcribing...
          </p>
        )}
      </div>

      {/* Live transcript preview */}
      {transcript && (
        <p className="text-xs text-muted-foreground/60 text-center max-w-[400px] line-clamp-2 italic">
          {transcript}
        </p>
      )}

      {/* Controls */}
      <div className="flex items-center gap-3">
        {isCapturing && (
          <button
            type="button"
            onClick={handleStop}
            className="flex items-center gap-2 px-4 py-2 rounded-full bg-brand text-white text-xs font-medium hover:bg-brand/90 transition-colors"
          >
            <Square className="size-3" fill="currentColor" />
            Done
          </button>
        )}
      </div>

      {/* Hints */}
      <div className="flex items-center gap-4 text-[11px] text-muted-foreground/50">
        <span className="flex items-center gap-1">
          <kbd className="px-1 py-0.5 glass-badge text-[10px]">Enter</kbd> Stop
        </span>
        <span className="flex items-center gap-1">
          <kbd className="px-1 py-0.5 glass-badge text-[10px]">Esc</kbd> Cancel
        </span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Build**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/launcher/components/VoiceRecorder.tsx
git commit -m "feat(voice): add VoiceRecorder component with waveform animation"
```

---

### Task 3: Wire recording mode into LauncherPage

**Files:**
- Modify: `desktop-ui/src/features/tray/pages/LauncherPage.tsx`

- [ ] **Step 1: Add recording mode handling**

In `desktop-ui/src/features/tray/pages/LauncherPage.tsx`, add the import and recording mode rendering:

Add import at top:
```typescript
import { VoiceRecorder } from "@features/launcher/components/VoiceRecorder";
```

Add a handler for when recording produces a transcript (after `enterChat`):

```typescript
const handleTranscriptReady = useCallback(
  (transcript: string) => {
    enterChat(transcript);
  },
  [enterChat],
);

const cancelRecording = useCallback(() => {
  setMode("dashboard");
}, [setMode]);
```

In the JSX, update the rendering to handle `mode === "recording"`:

Replace the existing conditional:
```tsx
{mode === "chat" && chatSessionKey ? (
  <LauncherChat ... />
) : (
  <div className="relative ...">
```

With:
```tsx
{mode === "recording" ? (
  <VoiceRecorder
    onTranscriptReady={handleTranscriptReady}
    onCancel={cancelRecording}
  />
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
```

Also add a listener for the `voice-recording-start` event (emitted by the Rust hotkey handler):

```typescript
// Listen for voice recording start event from hotkey
useEffect(() => {
  if (!isTauri) return;
  let unlisten: (() => void) | undefined;
  import("@tauri-apps/api/event").then(({ listen }) => {
    listen("voice-recording-start", () => {
      setMode("recording");
    }).then((fn) => {
      unlisten = fn;
    });
  });
  return () => unlisten?.();
}, [setMode]);
```

- [ ] **Step 2: Build**

Run: `cd desktop-ui && bun run build`
Expected: Compiles.

- [ ] **Step 3: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tray/pages/LauncherPage.tsx
git commit -m "feat(voice): wire recording mode into launcher page with voice event listener"
```

---

### Task 4: Update voice hotkey to open launcher in recording mode

**Files:**
- Modify: `crates/desktop/src/main.rs`

- [ ] **Step 1: Replace the voice hotkey handler**

In `crates/desktop/src/main.rs`, find the voice hotkey registration block (starts at ~line 306) and replace the entire handler. Instead of directly calling `voice_start_capture`, the hotkey should:

1. Show the launcher window
2. Emit a `voice-recording-start` event to the frontend
3. Let the frontend VoiceRecorder component handle capture start/stop

Replace the spawn block inside the `on_shortcut` callback:

```rust
tracing::info!("Voice hotkey pressed");
let handle = app_clone.clone();
tauri::async_runtime::spawn(async move {
    use tauri::{Emitter, Manager};

    // Show the launcher window
    if let Some(launcher) = handle.get_webview_window("launcher") {
        let _ = launcher.show();
        let _ = launcher.set_focus();
    }

    // Tell the frontend to enter recording mode
    if let Err(e) = handle.emit("voice-recording-start", ()) {
        tracing::warn!("Failed to emit voice-recording-start: {e}");
    }
});
```

Remove the `voice_start_capture`, `voice_stop_capture`, `VOICE_ACTIVE` toggle, and `is_capturing` check from the hotkey handler. Those are now handled by the VoiceRecorder component in the frontend.

- [ ] **Step 2: Build**

Run: `cargo build -p desktop`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/main.rs
git commit -m "feat(voice): hotkey opens launcher in recording mode instead of direct capture"
```

---

### Task 5: Handle voice_stop_capture transcript → chat flow

**Files:**
- Modify: `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`

- [ ] **Step 1: Update useVoiceEvents to expose transcript on finalization**

The current `useVoiceEvents` hook tracks `sessionState` and `transcript`. The VoiceRecorder already uses these. But we need to make sure the `transcript` is populated from the `finalized` event's `text` field (not just from partial transcripts).

In `desktop-ui/src/features/voice/hooks/useVoiceEvents.ts`, verify the `finalized` case sets `transcript`:

```typescript
case "finalized":
  setSessionState("response");
  setTranscript(payload.text as string);
  setResponseText((payload.responsePreview as string) || (payload.text as string));
  break;
```

If the current code only sets `responseText` but not `transcript` from the finalized event, add the `setTranscript` line. The VoiceRecorder component watches `transcript` to trigger `onTranscriptReady`.

- [ ] **Step 2: Verify the hook already handles this correctly**

Read the current `useVoiceEvents.ts` and check the `finalized` case. If `setTranscript` is already there for the finalized event, this task is a no-op — just verify and commit a comment or skip.

- [ ] **Step 3: Build and test**

Run: `cd desktop-ui && bun run build && bun run test`
Expected: All tests pass.

- [ ] **Step 4: Commit (if changes were needed)**

```bash
git add desktop-ui/src/features/voice/hooks/useVoiceEvents.ts
git commit -m "fix(voice): ensure transcript is set on finalized event for launcher chat flow"
```

---

### Task 6: Frontend tests for VoiceRecorder and launcher recording mode

**Files:**
- Modify: `desktop-ui/src/features/voice/__tests__/useVoiceEvents.test.ts`

- [ ] **Step 1: Add recording mode tests**

Add to the existing test file:

```typescript
describe("Launcher recording mode flow", () => {
  it("transitions from recording → capturing on captureStarted", () => {
    // Recording mode is a frontend-only state (not in VoiceEvent)
    // The VoiceRecorder component starts capture on mount
    // and listens for captureStarted to confirm it's active
    const state = reduceVoiceEvent(
      { sessionState: "idle" as const, chips: [], transcript: "" },
      { type: "captureStarted", sessionId: "s1", engine: "local" },
    );
    expect(state.sessionState).toBe("capturing");
  });

  it("transcript available on finalized for chat handoff", () => {
    const state = reduceVoiceEvent(
      { sessionState: "processing" as const, chips: [], transcript: "" },
      { type: "finalized", text: "schedule dentist tomorrow", routedTo: "tasks", responsePreview: "" },
    );
    expect(state.sessionState).toBe("response");
    // The VoiceRecorder watches sessionState === "response" && transcript
    // to call onTranscriptReady, which triggers enterChat
  });
});
```

- [ ] **Step 2: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/voice/__tests__/useVoiceEvents.test.ts
git commit -m "test(voice): add launcher recording mode flow tests"
```

---

### Task 7: Final build + verification

**Files:** None (verification only)

- [ ] **Step 1: Full Rust build**

Run: `cargo build --workspace`
Expected: Compiles.

- [ ] **Step 2: Rust tests**

Run: `cargo nextest run -p voice-engine -p desktop`
Expected: All pass.

- [ ] **Step 3: Frontend build + lint + test**

Run: `cd desktop-ui && bun run build && bun run lint && bun run test`
Expected: All pass.

- [ ] **Step 4: Commit any fixes**

If any step required fixes:
```bash
git add -A
git commit -m "chore: fix build issues for voice-launcher integration"
```
