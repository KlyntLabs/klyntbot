<script lang="ts">
  interface CalendarEvent {
    id: string;
    title: string;
    source: 'caldav' | 'todo';
    startTime?: string;
    endTime?: string;
    date?: string;
  }

  let {
    view = 'month',
    events = [],
    date = new Date(),
    onEventClick,
  }: {
    view?: 'month' | 'week' | 'day';
    events?: CalendarEvent[];
    date?: Date;
    onEventClick?: (event: CalendarEvent) => void;
  } = $props();

  let activePopover = $state<CalendarEvent | null>(null);

  const HOURS = Array.from({ length: 17 }, (_, i) => i + 7); // 7am to 11pm

  function getDaysInMonth(d: Date) {
    const year = d.getFullYear();
    const month = d.getMonth();
    const firstDay = new Date(year, month, 1).getDay();
    const daysInMonth = new Date(year, month + 1, 0).getDate();
    const cells: Array<number | null> = [];
    for (let i = 0; i < firstDay; i++) cells.push(null);
    for (let d = 1; d <= daysInMonth; d++) cells.push(d);
    return cells;
  }

  function getWeekDays(d: Date) {
    const day = d.getDay();
    const monday = new Date(d);
    monday.setDate(d.getDate() - day + (day === 0 ? -6 : 1));
    return Array.from({ length: 7 }, (_, i) => {
      const day = new Date(monday);
      day.setDate(monday.getDate() + i);
      return day;
    });
  }

  const monthCells = $derived(getDaysInMonth(date));
  const weekDays = $derived(getWeekDays(date));

  function handleEventClick(event: CalendarEvent) {
    activePopover = event;
    onEventClick?.(event);
  }

  function closePopover() {
    activePopover = null;
  }
</script>

<div class="calendar-view" data-testid="calendar-view" data-view={view}>
  <div class="view-toggle" role="group" aria-label="Calendar view">
    <button type="button" class:active={view === 'month'} onclick={() => view = 'month'}>Month</button>
    <button type="button" class:active={view === 'week'} onclick={() => view = 'week'}>Week</button>
    <button type="button" class:active={view === 'day'} onclick={() => view = 'day'}>Day</button>
  </div>

  {#if view === 'month'}
    <div class="month-grid" role="grid" aria-label="Month view">
      {#each ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as day}
        <div class="day-header" role="columnheader">{day}</div>
      {/each}
      {#each monthCells as cell}
        <div class="day-cell" role="gridcell" aria-label={cell ? `Day ${cell}` : 'empty'}>
          {#if cell}
            <span class="day-number">{cell}</span>
            {@const dayEvents = events.filter(e => e.date && new Date(e.date).getDate() === cell)}
            {#each dayEvents as evt}
              <button
                type="button"
                class="event-chip"
                class:caldav={evt.source === 'caldav'}
                class:todo={evt.source === 'todo'}
                onclick={() => handleEventClick(evt)}
              >
                {evt.title}
              </button>
            {/each}
          {/if}
        </div>
      {/each}
    </div>

  {:else if view === 'week'}
    <div class="week-grid" data-testid="week-grid">
      <div class="time-column"></div>
      {#each weekDays as day}
        <div class="week-day-header">{day.toLocaleDateString('en', { weekday: 'short', day: 'numeric' })}</div>
      {/each}
      {#each HOURS as hour}
        <div class="time-label">{hour}:00</div>
        {#each weekDays as _}
          <div class="time-slot" aria-label="{hour}:00"></div>
        {/each}
      {/each}
    </div>

  {:else}
    <div class="day-timeline" data-testid="day-timeline">
      {#each HOURS as hour}
        {@const hourEvents = events.filter(e => e.startTime && new Date(e.startTime).getHours() === hour)}
        <div class="hour-row">
          <span class="hour-label">{hour}:00</span>
          <div class="hour-events">
            {#each hourEvents as evt}
              <button
                type="button"
                class="event-card"
                class:caldav={evt.source === 'caldav'}
                class:todo={evt.source === 'todo'}
                onclick={() => handleEventClick(evt)}
              >
                {evt.title}
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if activePopover}
    <div class="event-popover" role="dialog" aria-label="Event details">
      <button type="button" class="close-btn" onclick={closePopover} aria-label="Close">×</button>
      <h3>{activePopover.title}</h3>
      <p>Source: {activePopover.source === 'caldav' ? 'Calendar' : 'Task'}</p>
      {#if activePopover.source === 'todo'}
        <a href="#task-{activePopover.id}" class="open-task-link">Open Task</a>
      {/if}
    </div>
  {/if}
</div>

<style>
  .calendar-view {
    padding: 16px;
    position: relative;
  }
  .view-toggle {
    display: flex;
    gap: 0;
    margin-bottom: 16px;
    border: 1px solid #30363D;
    border-radius: 6px;
    overflow: hidden;
    width: fit-content;
  }
  .view-toggle button {
    background: transparent;
    border: none;
    border-right: 1px solid #30363D;
    color: #8B949E;
    padding: 6px 16px;
    cursor: pointer;
    font-size: 14px;
  }
  .view-toggle button:last-child {
    border-right: none;
  }
  .view-toggle button.active {
    background: #FF8C00;
    color: #0D1117;
    font-weight: 600;
  }
  .month-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 1px;
    background: #21262D;
    border: 1px solid #21262D;
    border-radius: 8px;
    overflow: hidden;
  }
  .day-header {
    background: #161B22;
    padding: 8px;
    text-align: center;
    font-size: 12px;
    font-weight: 600;
    color: #8B949E;
  }
  .day-cell {
    background: #0D1117;
    padding: 8px;
    min-height: 80px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .day-number {
    font-size: 12px;
    color: #8B949E;
    margin-bottom: 4px;
  }
  .event-chip {
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 3px;
    border: none;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
    width: 100%;
  }
  .event-chip.caldav {
    background: rgba(88, 166, 255, 0.15);
    color: #58A6FF;
  }
  .event-chip.todo {
    background: rgba(255, 140, 0, 0.15);
    color: #FF8C00;
  }
  .week-grid {
    display: grid;
    grid-template-columns: 60px repeat(7, 1fr);
    gap: 1px;
  }
  .time-label {
    font-size: 11px;
    color: #6E7681;
    padding: 4px;
    text-align: right;
  }
  .time-slot {
    border: 1px solid #21262D;
    min-height: 40px;
  }
  .week-day-header {
    font-size: 12px;
    color: #8B949E;
    padding: 8px 4px;
    text-align: center;
  }
  .day-timeline {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .hour-row {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    min-height: 48px;
    border-bottom: 1px solid #21262D;
  }
  .hour-label {
    font-size: 11px;
    color: #6E7681;
    min-width: 40px;
    padding-top: 4px;
  }
  .hour-events {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .event-card {
    padding: 6px 10px;
    border-radius: 4px;
    border: none;
    cursor: pointer;
    text-align: left;
    font-size: 13px;
  }
  .event-card.caldav {
    background: rgba(88, 166, 255, 0.15);
    color: #58A6FF;
  }
  .event-card.todo {
    background: rgba(255, 140, 0, 0.15);
    color: #FF8C00;
  }
  .event-popover {
    position: absolute;
    top: 60px;
    left: 50%;
    transform: translateX(-50%);
    background: #21262D;
    border: 1px solid #30363D;
    border-radius: 8px;
    padding: 16px;
    min-width: 240px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    z-index: 100;
  }
  .event-popover h3 {
    margin: 0 0 8px;
    color: #E6EDF3;
  }
  .event-popover p {
    margin: 0 0 8px;
    color: #8B949E;
    font-size: 13px;
  }
  .close-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    background: transparent;
    border: none;
    color: #8B949E;
    font-size: 18px;
    cursor: pointer;
  }
  .open-task-link {
    color: #FF8C00;
    font-size: 13px;
  }
</style>
