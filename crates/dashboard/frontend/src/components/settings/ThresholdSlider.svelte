<script lang="ts">
  export let label: string;
  export let value: number = 0.5;
  export let min: number = 0;
  export let max: number = 1;
  export let step: number = 0.01;
  export let onChange: (v: number) => void = () => {};

  function handleInput(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    value = v;
    onChange(v);
  }

  $: displayValue = value.toFixed(2);
</script>

<div class="threshold-slider">
  <div class="header">
    <label for={`slider-${label}`}>{label}</label>
    <span class="value-display">{displayValue}</span>
  </div>
  <input
    id={`slider-${label}`}
    type="range"
    {min}
    {max}
    {step}
    {value}
    on:input={handleInput}
    role="slider"
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value}
    aria-valuetext={displayValue}
    aria-label={label}
  />
</div>

<style>
  .threshold-slider { display: flex; flex-direction: column; gap: 8px; }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  label { font-size: 14px; color: #E6EDF3; font-weight: 500; }

  .value-display {
    font-size: 13px;
    color: #FF8C00;
    font-family: 'JetBrains Mono', monospace;
    min-width: 40px;
    text-align: right;
  }

  input[type="range"] {
    width: 100%;
    accent-color: #FF8C00;
    cursor: pointer;
  }
</style>
