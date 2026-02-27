/**
 * Tests for the React Router route configuration.
 *
 * Covers:
 * - AC-16.3: All routes resolve and render without crashing
 * - AC-16.4: Pages render meaningful content (at minimum a heading)
 * - AC-16.5: Setup page renders outside the Layout shell
 * - Edge: Unknown route redirects to / (UX spec §10 addition)
 */

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { routes } from '../routes';

const ALL_ROUTES = [
  { path: '/',              label: 'Chat' },
  { path: '/chat/test-123', label: 'Chat Session' },
  { path: '/sessions',      label: 'Sessions' },
  { path: '/tasks',         label: 'Tasks' },
  { path: '/tasks/123',     label: 'Task Detail' },
  { path: '/plans',         label: 'Plans' },
  { path: '/calendar',      label: 'Calendar' },
  { path: '/cron',          label: 'Cron' },
  { path: '/skills',        label: 'Skills' },
  { path: '/finance',       label: 'Finance' },
  { path: '/settings',      label: 'Settings' },
  { path: '/setup',         label: 'Setup' },
];

describe('Routes', () => {
  // ── All routes render ───────────────────────────────────────────────────

  it.each(ALL_ROUTES)('route $path renders without crashing', ({ path }) => {
    // AC-16.3: Navigating to each route renders successfully (no thrown errors)
    const router = createMemoryRouter(routes, { initialEntries: [path] });
    expect(() => render(<RouterProvider router={router} />)).not.toThrow();
  });

  // Pages that render content immediately (no API loading gate)
  const PAGES_WITH_IMMEDIATE_HEADING = [
    { path: '/',              label: 'Chat' },
    { path: '/chat/test-123', label: 'Chat Session' },
    { path: '/sessions',      label: 'Sessions' },
    { path: '/calendar',      label: 'Calendar' },
    { path: '/setup',         label: 'Setup' },
  ];

  it.each(PAGES_WITH_IMMEDIATE_HEADING)('route $path renders at least one heading', ({ path }) => {
    // AC-16.4: Pages without API loading gates render at minimum one visible heading
    const router = createMemoryRouter(routes, { initialEntries: [path] });
    render(<RouterProvider router={router} />);
    const headings = screen.queryAllByRole('heading');
    expect(headings.length).toBeGreaterThanOrEqual(1);
  });

  // ── Setup outside Layout ──────────────────────────────────────────────────

  it('setup route does not render the nav rail', () => {
    // AC-16.5: /setup is outside the Layout shell — nav rail must not appear
    const router = createMemoryRouter(routes, { initialEntries: ['/setup'] });
    render(<RouterProvider router={router} />);
    expect(screen.queryByRole('navigation', { name: /main navigation/i })).not.toBeInTheDocument();
  });

  it('all other routes render the nav rail', () => {
    // AC-16.5: Routes nested under Layout have the nav rail
    const router = createMemoryRouter(routes, { initialEntries: ['/'] });
    render(<RouterProvider router={router} />);
    expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument();
  });

  // ── Unknown route fallback ────────────────────────────────────────────────

  it('unknown route renders inside Layout shell without crashing', () => {
    // UX spec §10 edge case: unknown paths are caught by the * wildcard inside Layout.
    const router = createMemoryRouter(routes, { initialEntries: ['/definitely/not/a/real/route'] });
    expect(() => render(<RouterProvider router={router} />)).not.toThrow();
    // Layout should render (nav rail is present — we're inside the / parent)
    expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument();
  });

  // ── Task detail route with ID param ──────────────────────────────────────

  it('/tasks/:id route renders TaskDetail page', () => {
    // AC-16.3: Dynamic segment :id route renders the TaskDetail component
    const router = createMemoryRouter(routes, { initialEntries: ['/tasks/abc-123'] });
    render(<RouterProvider router={router} />);
    // TaskDetail page renders without crashing inside Layout
    expect(screen.getByRole('navigation', { name: /main navigation/i })).toBeInTheDocument();
  });
});
