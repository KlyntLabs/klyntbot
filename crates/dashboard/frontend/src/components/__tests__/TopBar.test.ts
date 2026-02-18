/**
 * TopBar component tests — page header with breadcrumbs, view toggle, search, actions.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TopBar from '../TopBar.svelte';

describe('TopBar', () => {
  it('renders breadcrumb segments', () => {
    render(TopBar, {
      props: {
        breadcrumbs: [{ label: 'Tasks', href: '/tasks' }, { label: 'Board' }],
      },
    });
    expect(screen.getByText('Tasks')).toBeDefined();
    expect(screen.getByText('Board')).toBeDefined();
  });

  it('AC-03.5: renders view toggle for task pages', () => {
    render(TopBar, { props: { showViewToggle: true } });
    const toggle = screen.getByTestId('view-toggle');
    expect(toggle).toBeDefined();
    expect(screen.getByText('Board')).toBeDefined();
    expect(screen.getByText('List')).toBeDefined();
    expect(screen.getByText('Tree')).toBeDefined();
  });

  it('AC-03.5: view toggle switches active view', async () => {
    const onViewChange = vi.fn();
    render(TopBar, { props: { showViewToggle: true, activeView: 'board', onViewChange } });
    const listBtn = screen.getByRole('button', { name: 'List' });
    await fireEvent.click(listBtn);
    expect(onViewChange).toHaveBeenCalledWith('list');
  });

  it('renders action buttons', () => {
    const onClick = vi.fn();
    render(TopBar, { props: { actions: [{ label: '+ New Task', onClick }] } });
    expect(screen.getByRole('button', { name: '+ New Task' })).toBeDefined();
  });
});
