<script lang="ts">
  import StepItem from './StepItem.svelte';

  type StepStatus = 'Pending' | 'Running' | 'Completed' | 'Failed';

  interface Step {
    id: string;
    description: string;
    status: StepStatus;
    result?: string | null;
    attemptCount?: number;
    maxAttempts?: number;
    errorMessage?: string | null;
  }

  let {
    steps = [],
    expandedId = undefined,
  }: {
    steps?: Step[];
    expandedId?: string;
  } = $props();
</script>

<div class="step-list" role="list" aria-label="Plan steps">
  {#if steps.length === 0}
    <p class="empty">No steps defined</p>
  {:else}
    {#each steps as step, i (step.id)}
      <div role="listitem">
        <StepItem {step} index={i} />
      </div>
    {/each}
  {/if}
</div>

<style>
  .step-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .empty {
    color: #6E7681;
    font-size: 14px;
    padding: 16px;
  }
</style>
