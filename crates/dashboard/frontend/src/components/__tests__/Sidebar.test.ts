/**
 * Sidebar component tests — navigation panel.
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';

describe('Sidebar', () => {
  it('renders all navigation items', () => {
    render(Sidebar, {});
    expect(screen.getByText('Dashboard')).toBeDefined();
    expect(screen.getByText('Tasks')).toBeDefined();
    expect(screen.getByText('Projects')).toBeDefined();
    expect(screen.getByText('Goals')).toBeDefined();
    expect(screen.getByText('Calendar')).toBeDefined();
    expect(screen.getByText('Plans')).toBeDefined();
    expect(screen.getByText('Skills')).toBeDefined();
    expect(screen.getByText('Channels')).toBeDefined();
  });

  it('highlights active nav item', () => {
    render(Sidebar, { props: { activeHref: '/tasks' } });
    const tasksLink = screen.getByRole('link', { name: /tasks/i });
    expect(tasksLink.getAttribute('aria-current')).toBe('page');
  });

  it('shows badge count on Tasks nav item', () => {
    render(Sidebar, { props: { taskBadge: 12 } });
    const badge = screen.getByTestId('badge-tasks');
    expect(badge.textContent).toBe('12');
  });

  it('CC-3.1: has correct ARIA attributes', () => {
    render(Sidebar, {});
    const nav = screen.getByRole('navigation');
    expect(nav.getAttribute('aria-label')).toBe('Main navigation');
  });

  it('renders session list', () => {
    const sessions = [
      { id: '1', name: 'KlyntBot', lastMessage: 'hello', timestamp: '2m ago', active: true },
      { id: '2', name: 'Klynt', timestamp: '5m ago' },
      { id: '3', name: 'CGuard', timestamp: '1h ago' },
    ];
    render(Sidebar, { props: { sessions } });
    expect(screen.getByTestId('session-list')).toBeDefined();
    const items = screen.getAllByTestId('session-item');
    expect(items).toHaveLength(3);
  });

  it('notification bell shows unread count', () => {
    render(Sidebar, { props: { unreadNotifications: 3 } });
    const badge = screen.getByTestId('notification-badge');
    expect(badge.textContent).toBe('3');
    const bell = screen.getByRole('button', { name: /notifications.*3 unread/i });
    expect(bell).toBeDefined();
  });
});
