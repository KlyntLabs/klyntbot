<script lang="ts">
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
    step,
    index = 0,
  }: {
    step: Step;
    index?: number;
  } = $props();

  let resultExpanded = $state(false);
</script>

<div
  class="step-item"
  class:running={step.status === 'Running'}
  class:completed={step.status === 'Completed'}
  class:failed={step.status === 'Failed'}
  class:pending={step.status === 'Pending'}
  data-status={step.status}
>
  <div class="step-header">
    <div class="step-icon" aria-hidden="true">
      {#if step.status === 'Completed'}
        <span class="icon check">✓</span>
      {:else if step.status === 'Running'}
        <span class="icon spinner"></span>
      {:else if step.status === 'Failed'}
        <span class="icon x">✗</span>
      {:else}
        <span class="icon circle">○</span>
      {/if}
    </div>

    <div class="step-content">
      <span class="step-number">Step {index + 1}</span>
      <span class="step-description">{step.description}</span>
    </div>

    <div class="step-meta">
      <span class="step-status-label">{step.status}</span>
    </div>
  </div>

  {#if step.status === 'Failed' && step.errorMessage}
    <div class="error-block">
      <p class="error-message">{step.errorMessage}</p>
      {#if step.attemptCount}
        <span class="attempt-count">
          Attempt {step.attemptCount}/{step.maxAttempts ?? 3}
        </span>
      {/if}
    </div>
  {/if}

  {#if step.status === 'Completed' && step.result}
    <div class="result-block">
      <button
        type="button"
        class="toggle-result"
        onclick={() => resultExpanded = !resultExpanded}
        aria-expanded={resultExpanded}
      >
        Result {resultExpanded ? '▲' : '▼'}
      </button>
      {#if resultExpanded}
        <p class="result-text">{step.result}</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .step-item {
    padding: 12px 16px;
    border-radius: 6px;
    border: 1px solid #30363D;
    background: #161B22;
    transition: border-color 250ms ease;
  }
  .step-item.running {
    border-color: #58A6FF;
    animation: pulse 2s ease-in-out infinite;
  }
  .step-item.completed {
    border-color: #3FB950;
  }
  .step-item.failed {
    border-color: #F85149;
  }
  @keyframes pulse {
    0%, 100% { box-shadow: 0 0 0 0 rgba(88, 166, 255, 0); }
    50% { box-shadow: 0 0 0 4px rgba(88, 166, 255, 0.15); }
  }
  .step-header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .step-icon {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .icon {
    font-size: 16px;
  }
  .icon.check {
    color: #3FB950;
  }
  .icon.x {
    color: #F85149;
  }
  .icon.circle {
    color: #6E7681;
  }
  .icon.spinner {
    display: inline-block;
    width: 14px;
    height: 14px;
    border: 2px solid #30363D;
    border-top-color: #58A6FF;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
  .step-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .step-number {
    font-size: 11px;
    color: #6E7681;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .step-description {
    font-size: 14px;
    color: #E6EDF3;
  }
  .step-meta {
    display: flex;
    align-items: center;
  }
  .step-status-label {
    font-size: 12px;
    color: #8B949E;
  }
  .running .step-status-label { color: #58A6FF; }
  .completed .step-status-label { color: #3FB950; }
  .failed .step-status-label { color: #F85149; }
  .error-block {
    margin-top: 8px;
    padding: 8px 12px;
    background: rgba(248, 81, 73, 0.08);
    border-radius: 4px;
  }
  .error-message {
    font-size: 13px;
    color: #F85149;
    margin: 0 0 4px;
  }
  .attempt-count {
    font-size: 12px;
    color: #D29922;
  }
  .result-block {
    margin-top: 8px;
  }
  .toggle-result {
    background: transparent;
    border: none;
    color: #58A6FF;
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }
  .result-text {
    font-size: 13px;
    color: #8B949E;
    margin: 6px 0 0;
    padding: 8px;
    background: #0D1117;
    border-radius: 4px;
  }
</style>
