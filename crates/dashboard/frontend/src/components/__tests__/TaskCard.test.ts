/**
 * TaskCard component tests — individual task card in board/list views.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TaskCard from '../tasks/TaskCard.svelte';

const baseTask = {
  id: 't1',
  title: 'Fix auth bug',
  status: 'Todo' as const,
  priority: 2,
  tags: ['bug', 'auth'],
  dueDate: '2099-12-31T00:00:00Z',
};

describe('TaskCard', () => {
  it('renders task title, priority dot, and due date', () => {
    render(TaskCard, { props: { task: baseTask } });
    expect(screen.getByTestId('task-title').textContent).toBe('Fix auth bug');
    expect(screen.getByTestId('priority-dot')).toBeDefined();
    expect(screen.getByTestId('due-date')).toBeDefined();
  });

  it('renders tags', () => {
    render(TaskCard, { props: { task: baseTask } });
    expect(screen.getByText('bug')).toBeDefined();
    expect(screen.getByText('auth')).toBeDefined();
  });

  it('AC-03.3: click fires onClick handler', async () => {
    const onClick = vi.fn();
    render(TaskCard, { props: { task: baseTask, onClick } });
    const card = screen.getByTestId('task-card');
    await fireEvent.click(card);
    expect(onClick).toHaveBeenCalledWith(expect.objectContaining({ id: 't1' }));
  });

  it('AC-19.3: shows AI enriched badge when auto-enriched', () => {
    render(TaskCard, { props: { task: { ...baseTask, enriched: true } } });
    expect(screen.getByTestId('enriched-badge')).toBeDefined();
    expect(screen.getByTestId('enriched-badge').textContent).toContain('AI enriched');
  });

  it('AC-06.3: shows subtask progress indicator', () => {
    render(TaskCard, {
      props: { task: { ...baseTask, subtaskCount: 5, completedSubtasks: 2 } },
    });
    const progress = screen.getByTestId('subtask-progress');
    expect(progress.textContent).toContain('2/5');
  });

  it('renders focus timer indicator when active', () => {
    render(TaskCard, { props: { task: { ...baseTask, focusActive: true, focusElapsed: 1503 } } });
    expect(screen.getByTestId('focus-indicator')).toBeDefined();
  });

  it('CC-3.1: has correct ARIA label with task details', () => {
    render(TaskCard, { props: { task: baseTask } });
    const card = screen.getByTestId('task-card');
    expect(card.getAttribute('aria-label')).toContain('Fix auth bug');
    expect(card.getAttribute('aria-label')).toContain('priority high');
  });

  it('compact mode hides description and tags', () => {
    render(TaskCard, { props: { task: baseTask, compact: true } });
    const card = screen.getByTestId('task-card');
    expect(card.classList.contains('compact')).toBe(true);
  });
});
