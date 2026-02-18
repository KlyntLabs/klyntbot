<script lang="ts">
  import TaskForm from './TaskForm.svelte';

  let {
    open = false,
    onClose,
    onCreated,
    projects = [],
  } = $props<{
    open?: boolean;
    onClose?: () => void;
    onCreated?: (task: any) => void;
    projects?: { id: string; name: string }[];
  }>();

  let showToast = $state(false);
  let toastMessage = $state('');
  let toastTimeout: ReturnType<typeof setTimeout> | null = null;

  function showSuccess(msg: string) {
    toastMessage = msg;
    showToast = true;
    if (toastTimeout) clearTimeout(toastTimeout);
    toastTimeout = setTimeout(() => { showToast = false; }, 5000);
  }

  function handleSubmit(data: any) {
    onCreated?.(data);
    showSuccess('Task created');
    onClose?.();
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose?.();
  }
</script>

{#if open}
  <div
    class="modal-overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="modal-title"
    data-testid="task-create-modal"
    onkeydown={handleKeyDown}
  >
    <div class="modal-content">
      <div class="modal-header">
        <h2 id="modal-title" class="modal-title">New Task</h2>
        <button
          class="close-btn"
          onclick={onClose}
          aria-label="Close modal"
          data-testid="modal-close-btn"
        >✕</button>
      </div>
      <TaskForm {projects} onSubmit={handleSubmit} onCancel={onClose} />
    </div>
  </div>
{/if}

{#if showToast}
  <div
    class="toast"
    role="status"
    aria-live="polite"
    data-testid="toast"
  >
    {toastMessage}
    <button class="undo-btn" data-testid="undo-btn">Undo</button>
  </div>
{/if}

<style>
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center; z-index: 100; animation: slideUp 250ms cubic-bezier(0.34,1.56,0.64,1); }
  .modal-content { background: #161B22; border: 1px solid #30363D; border-radius: 12px; width: 100%; max-width: 640px; box-shadow: 0 16px 48px rgba(0,0,0,0.5); }
  .modal-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid #30363D; }
  .modal-title { font-size: 18px; color: #E6EDF3; font-weight: 600; margin: 0; }
  .close-btn { background: none; border: none; color: #8B949E; cursor: pointer; font-size: 16px; padding: 4px; }
  .toast { position: fixed; bottom: 24px; right: 24px; background: #21262D; border: 1px solid #30363D; border-radius: 8px; padding: 12px 16px; color: #E6EDF3; font-size: 14px; display: flex; align-items: center; gap: 12px; z-index: 200; }
  .undo-btn { background: none; border: none; color: #FF8C00; cursor: pointer; font-size: 14px; font-weight: 600; }
  @keyframes slideUp { from { transform: translateY(16px) scale(0.95); opacity: 0; } to { transform: translateY(0) scale(1); opacity: 1; } }
</style>
