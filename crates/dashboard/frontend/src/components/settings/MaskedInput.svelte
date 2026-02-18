<script lang="ts">
  export let value: string = '';
  export let revealed: boolean = false;
  export let onChange: (v: string) => void = () => {};

  $: maskedDisplay = revealed ? value : maskValue(value);

  function maskValue(v: string): string {
    if (!v) return '';
    const show = Math.min(10, Math.floor(v.length / 3));
    return '•'.repeat(v.length - show) + v.slice(-show);
  }

  function toggleReveal() { revealed = !revealed; }
</script>

<div class="masked-input">
  <input
    type={revealed ? 'text' : 'password'}
    {value}
    on:input={(e) => onChange((e.target as HTMLInputElement).value)}
    aria-label="API key"
    autocomplete="off"
    spellcheck="false"
  />
  <button
    class="reveal-btn"
    on:click={toggleReveal}
    aria-label={revealed ? 'Hide value' : 'Reveal value'}
    type="button"
  >
    {revealed ? '🙈' : '👁'}
  </button>
</div>

<style>
  .masked-input {
    display: flex;
    align-items: center;
    background: #0D1117;
    border: 1px solid #30363D;
    border-radius: 8px;
    overflow: hidden;
  }

  input {
    flex: 1;
    background: transparent;
    border: none;
    color: #E6EDF3;
    padding: 8px 12px;
    font-size: 14px;
    font-family: 'JetBrains Mono', monospace;
    outline: none;
  }

  .reveal-btn {
    background: transparent;
    border: none;
    border-left: 1px solid #30363D;
    padding: 8px 12px;
    cursor: pointer;
    font-size: 14px;
  }

  .reveal-btn:hover { background: #161B22; }

  .masked-input:focus-within {
    border-color: #FF8C00;
    box-shadow: 0 0 0 3px rgba(255, 140, 0, 0.3);
  }
</style>
