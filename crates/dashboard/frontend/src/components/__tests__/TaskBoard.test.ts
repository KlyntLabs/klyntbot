/**
 * TaskBoard component tests — Kanban board with drag-drop columns.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TaskBoard from '../tasks/TaskBoard.svelte';

const makeTasks = (counts: Record<string, number>) => {
  const tasks: any[] = [];
  let id = 1;
  for (const [status, count] of Object.entries(counts)) {
    for (let i = 0; i < count; i++) {
      tasks.push({ id: `t${id++}`, title: `Task ${id}`, status });
    }
  }
  return tasks;
};

describe('TaskBoard', () => {
  it('AC-03.1: renders three columns (Todo, Doing, Done)', () => {
    render(TaskBoard, { props: { tasks: [] } });
    expect(screen.getByTestId('column-todo')).toBeDefined();
    expect(screen.getByTestId('column-doing')).toBeDefined();
    expect(screen.getByTestId('column-done')).toBeDefined();
  });

  it('AC-03.1: distributes tasks to correct columns by status', () => {
    const tasks = makeTasks({ Todo: 3, Doing: 2, Done: 4 });
    render(TaskBoard, { props: { tasks } });
    const todoCol = screen.getByTestId('column-todo');
    const doingCol = screen.getByTestId('column-doing');
    const doneCol = screen.getByTestId('column-done');
    expect(todoCol.getAttribute('aria-label')).toContain('3 items');
    expect(doingCol.getAttribute('aria-label')).toContain('2 items');
    expect(doneCol.getAttribute('aria-label')).toContain('4 items');
  });

  it('AC-03.2: fires onDragEnd when card is dropped to new column', async () => {
    const onDragEnd = vi.fn();
    const tasks = [{ id: 't1', title: 'Task 1', status: 'Todo' }];
    render(TaskBoard, { props: { tasks, onDragEnd } });
    const card = screen.getByText('Task 1').closest('[data-task-id]')!;
    const doingCol = screen.getByTestId('column-doing');
    await fireEvent.dragStart(card);
    await fireEvent.dragOver(doingCol);
    await fireEvent.drop(doingCol);
    expect(onDragEnd).toHaveBeenCalledWith({ taskId: 't1', newStatus: 'Doing' });
  });

  it('CC-3.1: columns have correct ARIA attributes', () => {
    render(TaskBoard, { props: { tasks: [] } });
    const todoCol = screen.getByTestId('column-todo');
    expect(todoCol.getAttribute('role')).toBe('list');
    expect(todoCol.getAttribute('aria-label')).toMatch(/todo tasks/i);
    const board = screen.getByTestId('task-board');
    expect(board.getAttribute('role')).toBe('region');
    expect(board.getAttribute('aria-label')).toBe('Task board');
  });

  it('CC-3.2: keyboard drag-drop with Space and Arrow keys', async () => {
    const onTaskClick = vi.fn();
    const tasks = [{ id: 't1', title: 'My Task', status: 'Todo' }];
    render(TaskBoard, { props: { tasks, onTaskClick } });
    const card = screen.getByText('My Task').closest('[data-task-id]')!;
    await fireEvent.keyDown(card, { key: 'Enter' });
    expect(onTaskClick).toHaveBeenCalled();
  });

  it('shows empty state per column when no tasks', () => {
    render(TaskBoard, { props: { tasks: [] } });
    expect(screen.getByTestId('empty-todo')).toBeDefined();
    expect(screen.getByTestId('empty-doing')).toBeDefined();
    expect(screen.getByTestId('empty-done')).toBeDefined();
  });

  it('loading state shows skeleton cards', () => {
    render(TaskBoard, { props: { tasks: [], loading: true } });
    const skeletons = screen.getAllByTestId('skeleton');
    // 2 skeletons per column * 3 columns = 6
    expect(skeletons.length).toBeGreaterThanOrEqual(2);
  });
});
