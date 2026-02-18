<script lang="ts">
  let {
    taskId,
    elapsed = 0,
    running = false,
    onPause,
    onResume,
    onComplete,
  } = $props<{
    taskId: string;
    elapsed?: number;
    running?: boolean;
    onPause?: () => void;
    onResume?: () => void;
    onComplete?: (markDone: boolean) => void;
  }>();

  let currentElapsed = $state(elapsed);
  let isRunning = $state(running);
  let showCompletePrompt = $state(false);
  let interval: ReturnType<typeof setInterval> | null = null;

  function startTimer() {
    if (interval) clearInterval(interval);
    interval = setInterval(() => {
      currentElapsed += 1;
    }, 1000);
  }

  function stopTimer() {
    if (interval) {
      clearInterval(interval);
      interval = null;
    }
  }

  $effect(() => {
    if (isRunning) startTimer();
    else stopTimer();
    return () => stopTimer();
  });

  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }

  const minutes = $derived(Math.floor(currentElapsed / 60));
  const seconds = $derived(currentElapsed % 60);

  const ariaLabel = $derived(
    `Focus timer: ${minutes} minute${minutes !== 1 ? 's' : ''} ${seconds} second${seconds !== 1 ? 's' : ''}`
  );

  function handlePause() {
    isRunning = false;
    onPause?.();
  }

  function handleResume() {
    isRunning = true;
    onResume?.();
  }

  function handleComplete() {
    stopTimer();
    showCompletePrompt = true;
  }

  function confirmComplete(markDone: boolean) {
    showCompletePrompt = false;
    onComplete?.(markDone);
  }
</script>

<div
  class="focus-timer"
  role="timer"
  aria-live="off"
  aria-label={ariaLabel}
  data-testid="focus-timer"
>
  <div class="timer-display" data-testid="timer-display">
    {formatTime(currentElapsed)}
  </div>

  <div class="timer-controls">
    {#if isRunning}
      <button
        class="ctrl-btn"
        onclick={handlePause}
        aria-label="Pause timer"
        data-testid="pause-btn"
      >
        ⏸ Pause
      </button>
    {:else}
      <button
        class="ctrl-btn"
        onclick={handleResume}
        aria-label="Resume timer"
        data-testid="resume-btn"
      >
        ▶ Resume
      </button>
    {/if}

    <button
      class="ctrl-btn complete"
      onclick={handleComplete}
      aria-label="Complete focus session"
      data-testid="complete-btn"
    >
      ✓ Complete
    </button>
  </div>

  {#if showCompletePrompt}
    <div class="complete-prompt" data-testid="complete-prompt" role="alertdialog" aria-label="Complete session">
      <p>Mark task as done?</p>
      <div class="prompt-actions">
        <button class="btn-yes" onclick={() => confirmComplete(true)} data-testid="yes-btn">Yes</button>
        <button class="btn-no" onclick={() => confirmComplete(false)} data-testid="no-btn">No</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .focus-timer { background: #161B22; border: 1px solid #30363D; border-radius: 12px; padding: 16px; display: flex; flex-direction: column; align-items: center; gap: 12px; }
  .timer-display { font-size: 32px; font-weight: 700; color: #FF8C00; font-family: 'JetBrains Mono', monospace; }
  .timer-controls { display: flex; gap: 8px; }
  .ctrl-btn { background: #21262D; border: 1px solid #30363D; color: #E6EDF3; padding: 6px 14px; border-radius: 6px; cursor: pointer; font-size: 14px; }
  .ctrl-btn.complete { border-color: #3FB950; color: #3FB950; }
  .complete-prompt { background: #21262D; border: 1px solid #30363D; border-radius: 8px; padding: 12px 16px; text-align: center; }
  .complete-prompt p { color: #E6EDF3; margin: 0 0 12px; font-size: 14px; }
  .prompt-actions { display: flex; gap: 8px; justify-content: center; }
  .btn-yes { background: #3FB950; border: none; color: #0D1117; padding: 6px 20px; border-radius: 6px; cursor: pointer; font-weight: 600; }
  .btn-no { background: #21262D; border: 1px solid #30363D; color: #8B949E; padding: 6px 20px; border-radius: 6px; cursor: pointer; }
</style>
