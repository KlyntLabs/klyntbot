/**
 * Calendar component tests — CalendarView, SyncControls, ConflictList.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import CalendarView from '../calendar/CalendarView.svelte';
import SyncControls from '../calendar/SyncControls.svelte';
import ConflictList from '../calendar/ConflictList.svelte';

describe('CalendarView', () => {
  it('AC-11.1: renders month view with day cells', () => {
    render(CalendarView, { props: { view: 'month', events: [], date: new Date('2026-02-01') } });
    const grid = screen.getByRole('grid', { name: /Month view/i });
    expect(grid).toBeInTheDocument();
    // Month should have day cells
    const cells = screen.getAllByRole('gridcell');
    expect(cells.length).toBeGreaterThan(27); // at least 28 cells for a month
  });

  it('AC-11.1: renders week view with time grid', () => {
    render(CalendarView, { props: { view: 'week', events: [], date: new Date('2026-02-18') } });
    const weekGrid = document.querySelector('[data-testid="week-grid"]');
    expect(weekGrid).not.toBeNull();
  });

  it('AC-11.1: renders day view with timeline', () => {
    render(CalendarView, { props: { view: 'day', events: [], date: new Date('2026-02-18') } });
    const timeline = document.querySelector('[data-testid="day-timeline"]');
    expect(timeline).not.toBeNull();
  });

  it('AC-11.3: clicking event shows popover', async () => {
    const events = [
      { id: 'e1', title: 'Team Meeting', source: 'caldav' as const, date: '2026-02-01' },
    ];
    render(CalendarView, { props: { view: 'month', events, date: new Date('2026-02-01') } });
    const eventBtn = screen.getByRole('button', { name: /Team Meeting/i });
    await fireEvent.click(eventBtn);
    expect(screen.getByRole('dialog', { name: /Event details/i })).toBeInTheDocument();
    // Popover contains a heading with the event title
    expect(screen.getByRole('heading', { name: 'Team Meeting' })).toBeInTheDocument();
  });
});

describe('EventCard (via CalendarView)', () => {
  it('AC-11.2: CalDAV events colored blue', () => {
    const events = [{ id: 'e1', title: 'CalDAV Event', source: 'caldav' as const, date: '2026-02-01' }];
    render(CalendarView, { props: { view: 'month', events, date: new Date('2026-02-01') } });
    const btn = screen.getByRole('button', { name: /CalDAV Event/i });
    expect(btn.classList.contains('caldav')).toBe(true);
  });

  it('AC-11.2: Todo due dates colored orange', () => {
    const events = [{ id: 'e2', title: 'Task Due', source: 'todo' as const, date: '2026-02-01' }];
    render(CalendarView, { props: { view: 'month', events, date: new Date('2026-02-01') } });
    const btn = screen.getByRole('button', { name: /Task Due/i });
    expect(btn.classList.contains('todo')).toBe(true);
  });
});

describe('SyncControls', () => {
  it('AC-12.1: shows last sync time', () => {
    const lastSync = new Date(Date.now() - 2 * 60 * 1000); // 2 minutes ago
    render(SyncControls, { props: { status: 'idle', lastSync } });
    expect(screen.getByText(/Last synced: 2m ago/i)).toBeInTheDocument();
  });

  it('AC-12.2: sync now button triggers sync', async () => {
    const onSync = vi.fn();
    render(SyncControls, { props: { status: 'idle', lastSync: null, onSync } });
    const syncBtn = screen.getByRole('button', { name: /Sync now/i });
    await fireEvent.click(syncBtn);
    expect(onSync).toHaveBeenCalledOnce();
  });

  it('shows syncing spinner during sync', () => {
    render(SyncControls, { props: { status: 'syncing', lastSync: null } });
    // The status label shows "Syncing..."
    const statusLabel = document.querySelector('.status-label');
    expect(statusLabel?.textContent).toMatch(/Syncing/i);
    // The button is disabled
    const syncBtn = screen.getByRole('button');
    expect(syncBtn).toBeDisabled();
  });
});

describe('ConflictList', () => {
  const conflicts = [
    {
      id: 'c1',
      eventTitle: 'Team Sync',
      local: { title: 'Team Sync', time: '10:00 AM' },
      remote: { title: 'Team Sync Updated', time: '10:30 AM' },
    },
    {
      id: 'c2',
      eventTitle: 'Standup',
      local: { title: 'Standup' },
      remote: { title: 'Daily Standup' },
    },
  ];

  it('AC-12.3: renders conflict items with local vs remote diff', () => {
    render(ConflictList, { props: { conflicts } });
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(2);
    // Local vs remote diff text is visible (may appear multiple times across 2 items)
    expect(screen.getAllByText('Team Sync').length).toBeGreaterThan(0);
    expect(screen.getByText('Team Sync Updated')).toBeInTheDocument();
    // Both "Local" and "Remote" labels should be present
    expect(screen.getAllByText('Local').length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText('Remote').length).toBeGreaterThanOrEqual(2);
  });

  it('AC-12.3: resolve actions (Keep Local, Keep Remote, Merge)', async () => {
    const onResolve = vi.fn();
    render(ConflictList, { props: { conflicts: [conflicts[0]], onResolve } });
    const keepLocalBtn = screen.getByRole('button', { name: /Keep Local/i });
    await fireEvent.click(keepLocalBtn);
    expect(onResolve).toHaveBeenCalledWith({ id: 'c1', resolution: 'local' });
  });
});
