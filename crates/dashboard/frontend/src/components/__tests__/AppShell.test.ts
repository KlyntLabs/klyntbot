/**
 * AppShell component tests — main layout wrapper.
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import AppShell from '../AppShell.svelte';

describe('AppShell', () => {
  it('AC-01.1: renders sidebar, main content, and chat panel', () => {
    render(AppShell, { props: { sidebarOpen: true, chatOpen: true } });
    expect(screen.getByTestId('sidebar-panel')).toBeDefined();
    expect(screen.getByTestId('main-content')).toBeDefined();
    expect(screen.getByTestId('chat-panel')).toBeDefined();
  });

  it('sidebar has open class when sidebarOpen is true', () => {
    render(AppShell, { props: { sidebarOpen: true } });
    const sidebar = screen.getByTestId('sidebar-panel');
    expect(sidebar.classList.contains('open')).toBe(true);
    expect(sidebar.classList.contains('collapsed')).toBe(false);
  });

  it('toggles chat panel visibility', () => {
    render(AppShell, { props: { sidebarOpen: true, chatOpen: false } });
    expect(screen.queryByTestId('chat-panel')).toBeNull();
  });

  it('CC-2.1: sidebar has collapsed class when sidebarOpen is false', () => {
    // When sidebarOpen is false (collapsed), sidebar gets .collapsed class
    render(AppShell, { props: { sidebarOpen: false, chatOpen: false } });
    expect(screen.queryByTestId('chat-panel')).toBeNull();
    const sidebar = screen.getByTestId('sidebar-panel');
    expect(sidebar.classList.contains('collapsed')).toBe(true);
    expect(sidebar.classList.contains('open')).toBe(false);
  });
});
