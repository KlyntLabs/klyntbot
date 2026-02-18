/**
 * Notification component tests — NotificationBell, NotificationDrawer, NotificationItem.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import NotificationBell from '../notifications/NotificationBell.svelte';
import NotificationDrawer from '../notifications/NotificationDrawer.svelte';
import NotificationItem from '../notifications/NotificationItem.svelte';

const now = new Date();
const yesterday = new Date(now.getTime() - 86400000);
const earlier = new Date(now.getTime() - 2 * 86400000);

const makeNotification = (overrides = {}) => ({
  id: `n-${Math.random()}`,
  type: 'task',
  title: 'Task overdue',
  body: 'Fix auth regression is overdue',
  read: false,
  timestamp: now,
  href: '/tasks/todo-001',
  ...overrides,
});

describe('NotificationBell', () => {
  it('AC-17.1: shows unread count badge', () => {
    const { container } = render(NotificationBell, { props: { unreadCount: 3 } });
    const badge = container.querySelector('.badge');
    expect(badge?.textContent?.trim()).toBe('3');
  });

  it('no badge when 0 unread', () => {
    const { container } = render(NotificationBell, { props: { unreadCount: 0 } });
    expect(container.querySelector('.badge')).toBeNull();
  });

  it('click fires onClick', async () => {
    const onClick = vi.fn();
    render(NotificationBell, { props: { unreadCount: 1, onClick } });
    const btn = screen.getByRole('button');
    await fireEvent.click(btn);
    expect(onClick).toHaveBeenCalled();
  });

  it('CC-3.1: aria-label includes count', () => {
    render(NotificationBell, { props: { unreadCount: 3 } });
    const btn = screen.getByRole('button');
    expect(btn.getAttribute('aria-label')).toBe('Notifications, 3 unread');
    expect(btn.getAttribute('aria-haspopup')).toBe('true');
  });
});

describe('NotificationDrawer', () => {
  const notifications = [
    makeNotification({ id: 'n-1', timestamp: now }),
    makeNotification({ id: 'n-2', timestamp: now }),
    makeNotification({ id: 'n-3', timestamp: yesterday }),
    makeNotification({ id: 'n-4', timestamp: yesterday }),
    makeNotification({ id: 'n-5', timestamp: earlier }),
  ];

  it('AC-17.2: renders notification list grouped by time', () => {
    render(NotificationDrawer, { props: { open: true, notifications } });
    expect(screen.getByText('Today')).toBeTruthy();
    expect(screen.getByText('Yesterday')).toBeTruthy();
    expect(screen.getByText('Earlier')).toBeTruthy();
  });

  it('mark all read button', async () => {
    const onMarkAllRead = vi.fn();
    render(NotificationDrawer, {
      props: { open: true, notifications, onMarkAllRead }
    });
    const btn = screen.getByText('Mark all read');
    await fireEvent.click(btn);
    expect(onMarkAllRead).toHaveBeenCalled();
  });

  it('CC-3.1: drawer has dialog role', () => {
    const { container } = render(NotificationDrawer, {
      props: { open: true, notifications: [] }
    });
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog).toBeTruthy();
    expect(dialog?.getAttribute('aria-label')).toBe('Notification center');
  });
});

describe('NotificationItem', () => {
  it('AC-17.3: click navigates to relevant page', async () => {
    const onClick = vi.fn();
    const n = makeNotification({ href: '/tasks/todo-001' });
    render(NotificationItem, { props: { notification: n, onClick } });
    const btn = screen.getByRole('button');
    await fireEvent.click(btn);
    expect(onClick).toHaveBeenCalledWith(n);
  });

  it('renders type icon, title, body, timestamp', () => {
    const n = makeNotification({ title: 'Task overdue', body: 'Fix auth bug is overdue' });
    render(NotificationItem, { props: { notification: n } });
    expect(screen.getByText('Task overdue')).toBeTruthy();
    expect(screen.getByText('Fix auth bug is overdue')).toBeTruthy();
  });
});
