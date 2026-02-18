<script lang="ts">
  type Frequency = 'DAILY' | 'WEEKLY' | 'MONTHLY' | 'YEARLY';
  type EndType = 'never' | 'count' | 'until';

  export let rule: string = '';
  export let onChange: (rule: string) => void = () => {};

  const DAY_LABELS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  const DAY_KEYS = ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'];

  let frequency: Frequency = 'WEEKLY';
  let interval: number = 1;
  let selectedDays: string[] = [];
  let endType: EndType = 'never';
  let endCount: number = 10;
  let endDate: string = '';

  $: isWeekly = frequency === 'WEEKLY';

  $: rruleString = buildRRule(frequency, interval, selectedDays, endType, endCount, endDate);
  $: preview = buildPreview(frequency, interval, selectedDays, endType, endCount, endDate);

  $: onChange(rruleString);

  function buildRRule(freq: Frequency, int: number, days: string[], end: EndType, count: number, until: string): string {
    let rule = `RRULE:FREQ=${freq}`;
    if (int > 1) rule += `;INTERVAL=${int}`;
    if (freq === 'WEEKLY' && days.length > 0) rule += `;BYDAY=${days.join(',')}`;
    if (end === 'count') rule += `;COUNT=${count}`;
    if (end === 'until' && until) rule += `;UNTIL=${until.replace(/-/g, '')}`;
    return rule;
  }

  function buildPreview(freq: Frequency, int: number, days: string[], end: EndType, count: number, until: string): string {
    const freqWord = { DAILY: 'day', WEEKLY: 'week', MONTHLY: 'month', YEARLY: 'year' }[freq];
    let base = int === 1 ? `Every ${freqWord}` : `Every ${int} ${freqWord}s`;
    if (freq === 'WEEKLY' && days.length > 0) {
      const labels = days.map(d => DAY_LABELS[DAY_KEYS.indexOf(d)]).join(', ');
      base += ` on ${labels}`;
    }
    return base;
  }

  function toggleDay(day: string) {
    if (selectedDays.includes(day)) {
      selectedDays = selectedDays.filter(d => d !== day);
    } else {
      selectedDays = [...selectedDays, day];
    }
  }
</script>

<div class="rrule-builder">
  <div class="field-row">
    <label for="rrule-freq">Frequency</label>
    <select id="rrule-freq" bind:value={frequency}>
      <option value="DAILY">Daily</option>
      <option value="WEEKLY">Weekly</option>
      <option value="MONTHLY">Monthly</option>
      <option value="YEARLY">Yearly</option>
    </select>
  </div>

  <div class="field-row">
    <label for="rrule-interval">Every</label>
    <input
      id="rrule-interval"
      type="number"
      min="1"
      max="99"
      bind:value={interval}
    />
    <span class="unit">
      {{ DAILY: 'day(s)', WEEKLY: 'week(s)', MONTHLY: 'month(s)', YEARLY: 'year(s)' }[frequency]}
    </span>
  </div>

  {#if isWeekly}
    <div class="field-row">
      <label>On days</label>
      <div class="day-selector" role="group" aria-label="Days of the week">
        {#each DAY_KEYS as day, i}
          <button
            class="day-btn"
            class:selected={selectedDays.includes(day)}
            on:click={() => toggleDay(day)}
            aria-pressed={selectedDays.includes(day)}
            type="button"
          >
            {DAY_LABELS[i]}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="field-row">
    <label for="rrule-end">Ends</label>
    <select id="rrule-end" bind:value={endType}>
      <option value="never">Never</option>
      <option value="count">After N occurrences</option>
      <option value="until">On date</option>
    </select>
  </div>

  {#if endType === 'count'}
    <div class="field-row">
      <label for="rrule-count">After</label>
      <input id="rrule-count" type="number" min="1" bind:value={endCount} />
      <span class="unit">occurrences</span>
    </div>
  {:else if endType === 'until'}
    <div class="field-row">
      <label for="rrule-until">Until</label>
      <input id="rrule-until" type="date" bind:value={endDate} />
    </div>
  {/if}

  <div class="preview">
    <span class="preview-label">Preview:</span>
    <span class="preview-text">{preview}</span>
  </div>
</div>

<style>
  .rrule-builder {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px;
    background: #161B22;
    border: 1px solid #30363D;
    border-radius: 8px;
  }

  .field-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  label {
    font-size: 13px;
    color: #8B949E;
    min-width: 80px;
    font-weight: 500;
  }

  select, input[type="number"], input[type="date"] {
    background: #0D1117;
    border: 1px solid #30363D;
    border-radius: 6px;
    color: #E6EDF3;
    padding: 6px 10px;
    font-size: 13px;
  }

  input[type="number"] { width: 70px; }

  .unit { font-size: 13px; color: #6E7681; }

  .day-selector {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .day-btn {
    background: #21262D;
    border: 1px solid #30363D;
    border-radius: 6px;
    color: #8B949E;
    padding: 4px 8px;
    font-size: 12px;
    cursor: pointer;
    font-weight: 500;
  }

  .day-btn.selected, .day-btn[aria-pressed="true"] {
    background: rgba(255, 140, 0, 0.15);
    border-color: #FF8C00;
    color: #FF8C00;
  }

  .preview {
    background: #0D1117;
    border-radius: 6px;
    padding: 10px 12px;
    font-size: 13px;
  }

  .preview-label { color: #6E7681; margin-right: 8px; }
  .preview-text { color: #E6EDF3; font-style: italic; }
</style>
