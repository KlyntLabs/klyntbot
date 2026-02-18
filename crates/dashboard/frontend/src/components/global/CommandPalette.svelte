<script lang="ts">
  export interface PaletteItem {
    id: string;
    label: string;
    group: 'Navigation' | 'Tasks' | 'Projects' | 'Actions';
    action: () => void;
  }

  export let open: boolean = false;
  export let items: PaletteItem[] = [];
  export let onClose: () => void = () => {};

  let query = '';
  let selectedIndex = 0;
  let inputEl: HTMLInputElement;

  $: filteredItems = query
    ? items.filter(i => i.label.toLowerCase().includes(query.toLowerCase()))
    : items;

  $: groupedItems = Object.entries(
    filteredItems.reduce((acc, item) => {
      (acc[item.group] ??= []).push(item);
      return acc;
    }, {} as Record<string, PaletteItem[]>)
  );

  $: flatItems = filteredItems;

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { onClose(); return; }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, flatItems.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      flatItems[selectedIndex]?.action();
      onClose();
    }
  }

  $: if (open && inputEl) { setTimeout(() => inputEl?.focus(), 10); }
  $: if (!open) { query = ''; selectedIndex = 0; }
</script>

<svelte:window on:keydown={(e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    open = !open;
    if (!open) onClose();
  }
}} />

{#if open}
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <div class="backdrop" on:click={onClose} role="presentation" />
  <div
    class="command-palette"
    role="dialog"
    aria-label="Command palette"
    aria-modal="true"
  >
    <input
      bind:this={inputEl}
      bind:value={query}
      on:keydown={handleKeydown}
      type="text"
      placeholder="Type to search commands, pages, tasks..."
      aria-label="Search commands"
      role="combobox"
      aria-expanded={filteredItems.length > 0}
      aria-autocomplete="list"
      aria-controls="palette-results"
    />

    <div id="palette-results" class="results" role="listbox">
      {#if filteredItems.length === 0}
        <div class="no-results">No results found</div>
      {:else}
        {#each groupedItems as [group, groupItems]}
          <div class="group-label">{group}</div>
          {#each groupItems as item}
            {@const flatIdx = flatItems.indexOf(item)}
            <button
              class="result-item"
              class:selected={flatIdx === selectedIndex}
              role="option"
              aria-selected={flatIdx === selectedIndex}
              on:click={() => { item.action(); onClose(); }}
              on:mouseenter={() => selectedIndex = flatIdx}
            >
              {item.label}
            </button>
          {/each}
        {/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 200;
  }

  .command-palette {
    position: fixed;
    top: 20%;
    left: 50%;
    transform: translateX(-50%);
    width: 560px;
    max-width: calc(100vw - 32px);
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    z-index: 201;
    overflow: hidden;
  }

  input {
    width: 100%;
    background: transparent;
    border: none;
    border-bottom: 1px solid #30363D;
    color: #E6EDF3;
    padding: 16px 20px;
    font-size: 15px;
    outline: none;
    box-sizing: border-box;
  }

  input::placeholder { color: #6E7681; }

  .results { max-height: 400px; overflow-y: auto; }

  .no-results {
    padding: 16px 20px;
    color: #6E7681;
    font-size: 14px;
    text-align: center;
  }

  .group-label {
    font-size: 11px;
    font-weight: 600;
    color: #6E7681;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 8px 20px 4px;
  }

  .result-item {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: #E6EDF3;
    padding: 10px 20px;
    cursor: pointer;
    font-size: 14px;
  }

  .result-item:hover, .result-item.selected {
    background: rgba(255, 140, 0, 0.1);
    color: #FF8C00;
  }
</style>
