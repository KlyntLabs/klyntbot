/**
 * TaskDetail component tests — full task detail panel.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TaskDetail from '../tasks/TaskDetail.svelte';

const baseTask = {
  id: 't1',
  title: 'Fix auth regression',
  description: 'Users unable to login after session token update.',
  status: 'Doing' as const,
  priority: 1,
  tags: ['bug', 'auth', 'urgent'],
  dueDate: '2026-02-20T00:00:00Z',
  parentId: null,
  projectId: 'proj-001',
  attachments: [
    { id: 'att-001', type: 'file' as const, title: 'error_screenshot.png', path: '/tmp/error.png' },
  ],
  timeEntries: [
    { id: 'te-001', minutes: 25, note: 'Initial investigation', createdAt: '2026-02-18T09:00:00Z' },
    { id: 'te-002', minutes: 30, note: 'Deep dive', createdAt: '2026-02-18T10:00:00Z' },
  ],
  dependencies: ['Deploy v2'],
  subtasks: [
    { id: 's1', title: 'Reproduce bug', status: 'Done' as const },
    { id: 's2', title: 'Write failing test', status: 'Done' as const },
    { id: 's3', title: 'Fix token validation', status: 'Todo' as const },
    { id: 's4', title: 'Add regression test', status: 'Todo' as const },
  ],
};

describe('TaskDetail', () => {
  it('AC-03.3: renders all task fields', () => {
    render(TaskDetail, { props: { task: baseTask } });
    expect(screen.getByTestId('task-title-display')).toBeDefined();
    expect(screen.getByTestId('status-select')).toBeDefined();
    expect(screen.getByTestId('priority-badge')).toBeDefined();
    expect(screen.getByTestId('due-date')).toBeDefined();
    expect(screen.getByTestId('tags-list')).toBeDefined();
    expect(screen.getByTestId('description')).toBeDefined();
  });

  it('AC-03.4: inline title editing on click', async () => {
    render(TaskDetail, { props: { task: baseTask } });
    const titleText = screen.getByTestId('title-text');
    await fireEvent.click(titleText);
    expect(screen.getByTestId('title-input')).toBeDefined();
    const input = screen.getByTestId('title-input') as HTMLInputElement;
    await fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByTestId('title-input')).toBeNull();
  });

  it('AC-03.4: inline status editing via dropdown', () => {
    render(TaskDetail, { props: { task: baseTask } });
    const select = screen.getByTestId('status-select') as HTMLSelectElement;
    expect(select.value).toBe('Doing');
  });

  it('AC-06.1: subtask section shows nested list', () => {
    render(TaskDetail, { props: { task: baseTask } });
    const subtaskList = screen.getByTestId('subtask-list');
    const items = screen.getAllByTestId('subtask-item');
    expect(items).toHaveLength(4);
  });

  it('AC-06.2: add subtask inline form', async () => {
    render(TaskDetail, { props: { task: baseTask } });
    const addBtn = screen.getByTestId('add-subtask-btn');
    await fireEvent.click(addBtn);
    expect(screen.getByTestId('subtask-input')).toBeDefined();
    const input = screen.getByTestId('subtask-input');
    await fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByTestId('subtask-input')).toBeNull();
  });

  it('AC-06.3: subtask progress indicator', () => {
    render(TaskDetail, { props: { task: baseTask } });
    const progress = screen.getByTestId('subtask-progress');
    expect(progress.textContent).toContain('2/4');
  });

  it('renders attachments section', () => {
    render(TaskDetail, { props: { task: baseTask } });
    expect(screen.getByTestId('attachments-section')).toBeDefined();
    const attachments = screen.getAllByTestId('attachment-item');
    expect(attachments).toHaveLength(1);
    expect(attachments[0].textContent).toContain('error_screenshot.png');
  });

  it('renders time entries section', () => {
    render(TaskDetail, { props: { task: baseTask } });
    const section = screen.getByTestId('time-section');
    expect(section.textContent).toContain('55m');
  });

  it('renders dependencies section', () => {
    render(TaskDetail, { props: { task: baseTask } });
    const section = screen.getByTestId('dependencies-section');
    expect(section.textContent).toContain('Deploy v2');
  });

  it('close button fires onClose', async () => {
    const onClose = vi.fn();
    render(TaskDetail, { props: { task: baseTask, onClose } });
    const closeBtn = screen.getByTestId('close-btn');
    await fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });
});
