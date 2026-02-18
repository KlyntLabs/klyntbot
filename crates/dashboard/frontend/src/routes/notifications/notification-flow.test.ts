/**
 * Integration tests for the notification flow.
 * Tests bell badge, drawer open/close, grouping, and mark-as-read.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import NotificationBell from '../../components/notifications/NotificationBell.svelte';
import NotificationDrawer from '../../components/notifications/NotificationDrawer.svelte';
import NotificationItem from '../../components/notifications/NotificationItem.svelte';
import type { Notification } from '../../lib/types/index';

const now = new Date();
const yesterday = new Date(now.getTime() - 86400000);
const twoDaysAgo = new Date(now.getTime() - 2 * 86400000);

const mockNotifications: Notification[] = [
  {
    id: 'n-1',
    type: 'overdue',
    title: 'Task overdue',
    body: 'Fix auth regression is overdue',
    read: false,
    timestamp: now,
    href: '/tasks/todo-001',
  },
  {
    id: 'n-2',
    type: 'plan',
    title: 'Plan completed',
    body: 'Deploy v2.0 completed successfully',
    read: false,
    timestamp: now,
    href: '/plans/plan-001',
  },
  {
    id: 'n-3',
    type: 'calendar',
    title: 'Calendar synced',
    body: '12 events synced from CalDAV',
    read: true,
    timestamp: yesterday,
    href: '/calendar',
  },
  {
    id: 'n-4',
    type: 'digest',
    title: 'Daily digest',
    body: 'You completed 3 tasks today',
    read: true,
    timestamp: yesterday,
    href: '/dashboard',
  },
  {
    id: 'n-5',
    type: 'focus',
    title: 'Focus session reminder',
    body: 'You have a pending focus session',
    read: true,
    timestamp: twoDaysAgo,
    href: '/tasks',
  },
];

describe('Notification Flow Integration', () => {
  it('AC-17.1: badge increments for each unread notification', () => {
    const unreadCount = mockNotifications.filter(n => !n.read).length;
    const { container } = render(NotificationBell, { props: { unreadCount } });

    const badge = container.querySelector('.badge');
    expect(badge?.textContent?.trim()).toBe(String(unreadCount));
    expect(unreadCount).toBe(2);
  });

  it('AC-17.1: badge hidden when no unread notifications', () => {
    const { container } = render(NotificationBell, { props: { unreadCount: 0 } });
    expect(container.querySelector('.badge')).toBeNull();
  });

  it('AC-17.2: drawer groups notifications by Today/Yesterday/Earlier', () => {
    render(NotificationDrawer, { props: { open: true, notifications: mockNotifications } });

    expect(screen.getByText('Today')).toBeTruthy();
    expect(screen.getByText('Yesterday')).toBeTruthy();
    expect(screen.getByText('Earlier')).toBeTruthy();
  });

  it('AC-17.2: mark all read fires callback', async () => {
    const onMarkAllRead = vi.fn();
    render(NotificationDrawer, {
      props: { open: true, notifications: mockNotifications, onMarkAllRead }
    });

    await fireEvent.click(screen.getByText('Mark all read'));
    expect(onMarkAllRead).toHaveBeenCalledTimes(1);
  });

  it('AC-17.3: notification item click passes notification to handler', async () => {
    const onClick = vi.fn();
    const n = mockNotifications[0];
    render(NotificationItem, { props: { notification: n, onClick } });

    await fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledWith(n);
  });

  it('unread notification has distinct visual style', () => {
    const unread = { ...mockNotifications[0], read: false };
    const { container: c1 } = render(NotificationItem, { props: { notification: unread } });

    const read = { ...mockNotifications[2], read: true };
    const { container: c2 } = render(NotificationItem, { props: { notification: read } });

    expect(c1.querySelector('.unread')).toBeTruthy();
    expect(c2.querySelector('.unread')).toBeNull();
  });

  it('drawer closed when open=false', () => {
    const { container } = render(NotificationDrawer, {
      props: { open: false, notifications: mockNotifications }
    });
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it('drawer accessible: role=dialog with correct label', () => {
    const { container } = render(NotificationDrawer, {
      props: { open: true, notifications: [] }
    });
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog?.getAttribute('aria-label')).toBe('Notification center');
    expect(dialog?.getAttribute('aria-modal')).toBe('true');
  });
});
