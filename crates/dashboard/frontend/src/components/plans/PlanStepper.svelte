<script lang="ts">
  type PlanStatus = 'Draft' | 'Approved' | 'Executing' | 'Completed' | 'Failed' | 'Abandoned';

  const LIFECYCLE = ['Draft', 'Approved', 'Executing', 'Completed'] as const;

  const STATUS_STEP: Record<string, number> = {
    Draft: 0,
    Approved: 1,
    Executing: 2,
    Completed: 3,
    Failed: 2,
    Abandoned: -1,
  };

  interface Plan {
    id: string;
    title: string;
    status: PlanStatus;
  }

  let {
    plan,
    onApprove,
    onExecute,
    onAbandon,
  }: {
    plan: Plan;
    onApprove?: () => void;
    onExecute?: () => void;
    onAbandon?: () => void;
  } = $props();

  const currentStep = $derived(STATUS_STEP[plan.status] ?? 0);
  const progressValue = $derived(Math.max(0, currentStep));
</script>

<div class="plan-stepper">
  <div
    class="status-bar"
    role="progressbar"
    aria-valuenow={progressValue}
    aria-valuemin={0}
    aria-valuemax={3}
    aria-label="Plan lifecycle progress"
  >
    {#each LIFECYCLE as stage, i}
      <div
        class="stage"
        class:active={i === currentStep && plan.status !== 'Failed' && plan.status !== 'Abandoned'}
        class:completed={i < currentStep || (i === currentStep && plan.status === 'Completed')}
        class:failed={plan.status === 'Failed' && i === currentStep}
        aria-label="{stage}{i === currentStep ? ' (current)' : ''}"
      >
        <div class="stage-dot"></div>
        <span class="stage-label">{stage}</span>
      </div>
      {#if i < LIFECYCLE.length - 1}
        <div class="stage-connector" class:filled={i < currentStep}></div>
      {/if}
    {/each}
  </div>

  {#if plan.status === 'Abandoned' || plan.status === 'Failed'}
    <p class="status-badge" class:failed={plan.status === 'Failed'}>
      {plan.status}
    </p>
  {/if}

  <div class="actions">
    {#if plan.status === 'Draft'}
      <button type="button" class="btn approve" onclick={() => onApprove?.()}>Approve</button>
      <button type="button" class="btn abandon" onclick={() => onAbandon?.()}>Abandon</button>
    {:else if plan.status === 'Approved'}
      <button type="button" class="btn execute" onclick={() => onExecute?.()}>Execute</button>
      <button type="button" class="btn abandon" onclick={() => onAbandon?.()}>Abandon</button>
    {:else if plan.status === 'Executing'}
      <button type="button" class="btn abandon" onclick={() => onAbandon?.()}>Abandon</button>
    {/if}
  </div>
</div>

<style>
  .plan-stepper {
    padding: 16px 0;
  }
  .status-bar {
    display: flex;
    align-items: center;
    margin-bottom: 20px;
  }
  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .stage-dot {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid #30363D;
    background: #0D1117;
    transition: all 250ms ease;
  }
  .stage.active .stage-dot {
    border-color: #FF8C00;
    background: #FF8C00;
    box-shadow: 0 0 0 4px rgba(255, 140, 0, 0.2);
  }
  .stage.completed .stage-dot {
    border-color: #3FB950;
    background: #3FB950;
  }
  .stage.failed .stage-dot {
    border-color: #F85149;
    background: #F85149;
  }
  .stage-label {
    font-size: 11px;
    color: #6E7681;
    text-align: center;
    white-space: nowrap;
  }
  .stage.active .stage-label {
    color: #FF8C00;
    font-weight: 600;
  }
  .stage.completed .stage-label {
    color: #3FB950;
  }
  .stage-connector {
    flex: 1;
    height: 2px;
    background: #30363D;
    margin-bottom: 18px;
    transition: background 250ms ease;
  }
  .stage-connector.filled {
    background: #3FB950;
  }
  .status-badge {
    font-size: 12px;
    color: #6E7681;
    margin: 0 0 12px;
    font-weight: 600;
  }
  .status-badge.failed {
    color: #F85149;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn {
    border-radius: 6px;
    padding: 6px 16px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid;
    background: transparent;
  }
  .btn.approve {
    color: #3FB950;
    border-color: #3FB950;
  }
  .btn.execute {
    color: #FF8C00;
    border-color: #FF8C00;
  }
  .btn.abandon {
    color: #F85149;
    border-color: #F85149;
  }
</style>
