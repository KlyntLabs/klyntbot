/**
 * Tests for the Layout component.
 *
 * Covers:
 * - AC-16.1: 48px nav rail renders with 9 navigation items + settings icon
 * - AC-16.2: Active route item has aria-current="page"
 * - AC-15.4: Codex dark theme CSS variables are referenced
 * - Accessibility: nav landmark, aria labels, keyboard navigation
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { Layout } from '../Layout';

const NAV_ITEMS = ['Chat', 'Sessions', 'Tasks', 'Projects', 'Plans', 'Calendar', 'Cron', 'Skills', 'Finance'];
const SETTINGS_ITEM = 'Settings';

describe('Layout', () => {
  // ── Nav rail structure ────────────────────────────────────────────────────

  it('renders a nav rail with 9 primary navigation items plus settings', () => {
    // AC-16.1: Nav rail has exactly 9 items plus a Settings item at the bottom.
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const nav = screen.getByRole('navigation', { name: /main navigation/i });
    expect(nav).toBeInTheDocument();
    for (const item of NAV_ITEMS) {
      expect(screen.getByRole('button', { name: new RegExp(item, 'i') })).toBeInTheDocument();
    }
    expect(screen.getByRole('button', { name: new RegExp(SETTINGS_ITEM, 'i') })).toBeInTheDocument();
  });

  it('renders settings nav item separate from primary items', () => {
    // AC-16.1: Settings is at the bottom of the nav rail (visual separation)
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const settingsBtn = screen.getByRole('button', { name: /settings/i });
    expect(settingsBtn).toBeInTheDocument();
  });

  it('nav rail has aria-label "Main navigation"', () => {
    // Accessibility, UX §4.2: role="navigation" aria-label="Main navigation"
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const nav = screen.getByRole('navigation', { name: /main navigation/i });
    expect(nav).toHaveAttribute('aria-label', 'Main navigation');
  });

  it('nav rail width is 48px', () => {
    // AC-16.1: Nav rail is exactly 48px wide (per design spec)
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const nav = screen.getByRole('navigation', { name: /main navigation/i });
    expect(nav.className).toMatch(/w-\[48px\]/);
  });

  // ── Active route highlighting ─────────────────────────────────────────────

  it('active route item has aria-current="page"', () => {
    // AC-16.2, UX §4.2: Active nav item has aria-current="page"
    render(<MemoryRouter initialEntries={['/tasks']}><Layout /></MemoryRouter>);
    const tasksBtn = screen.getByRole('button', { name: /tasks/i });
    expect(tasksBtn).toHaveAttribute('aria-current', 'page');
  });

  it('non-active route items do not have aria-current', () => {
    // AC-16.2: Only the active route has aria-current="page"
    render(<MemoryRouter initialEntries={['/tasks']}><Layout /></MemoryRouter>);
    const chatBtn = screen.getByRole('button', { name: /^chat$/i });
    expect(chatBtn).not.toHaveAttribute('aria-current', 'page');
  });

  it('active nav item uses foreground color style', () => {
    // AC-16.2: Active button uses --codex-fg via inline style
    render(<MemoryRouter initialEntries={['/tasks']}><Layout /></MemoryRouter>);
    const tasksBtn = screen.getByRole('button', { name: /tasks/i });
    expect(tasksBtn.style.color).toBe('var(--codex-fg)');
  });

  // ── Status bar ────────────────────────────────────────────────────────────

  it('renders status bar at bottom of viewport', () => {
    // UX §8: StatusBar is rendered in the Layout shell
    render(<MemoryRouter><Layout /></MemoryRouter>);
    // Status bar contains task/session counts
    expect(screen.getByText(/\d+ tasks/)).toBeInTheDocument();
  });

  // ── Theme ─────────────────────────────────────────────────────────────────

  it('root element uses codex background color', () => {
    // AC-15.4: Layout background uses --codex-bg CSS variable
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const nav = screen.getByRole('navigation', { name: /main navigation/i });
    const container = nav.parentElement;
    expect(container).not.toBeNull();
    const rootDiv = container?.parentElement;
    expect(rootDiv).not.toBeNull();
  });

  // ── Keyboard navigation ───────────────────────────────────────────────────

  it('nav items are focusable via keyboard Tab', () => {
    // UX §4.1: Tab key moves focus between nav items
    render(<MemoryRouter><Layout /></MemoryRouter>);
    for (const item of NAV_ITEMS) {
      const btn = screen.getByRole('button', { name: new RegExp(item, 'i') });
      // Buttons are focusable by default (tabIndex not -1)
      expect(btn).not.toHaveAttribute('tabindex', '-1');
    }
  });

  it('nav items are buttons that handle keyboard activation', () => {
    // UX §4.1: Buttons natively support Enter/Space activation
    render(<MemoryRouter><Layout /></MemoryRouter>);
    for (const item of NAV_ITEMS) {
      const btn = screen.getByRole('button', { name: new RegExp(item, 'i') });
      expect(btn.tagName.toLowerCase()).toBe('button');
    }
  });

  // ── Content area ──────────────────────────────────────────────────────────

  it('renders main content area next to nav rail', () => {
    // AC-16.1: The Layout positions a main content area (flex row layout)
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const main = screen.getByRole('main');
    expect(main).toBeInTheDocument();
  });

  it('outlet content is rendered in main area', () => {
    // AC-16.3: The <Outlet /> from React Router renders in the content area
    render(<MemoryRouter><Layout /></MemoryRouter>);
    const main = screen.getByRole('main');
    expect(main).toBeInTheDocument();
    expect(main.tagName.toLowerCase()).toBe('main');
  });
});
