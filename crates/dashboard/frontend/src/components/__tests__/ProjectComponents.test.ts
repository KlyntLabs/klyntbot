/**
 * Project component tests — ProjectGrid, ProjectCard, ProjectDetail, ProjectForm.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ProjectGrid from '../projects/ProjectGrid.svelte';
import ProjectCard from '../projects/ProjectCard.svelte';
import ProjectDetail from '../projects/ProjectDetail.svelte';
import ProjectForm from '../projects/ProjectForm.svelte';

const sampleProject = {
  id: 'proj-001',
  name: 'Klyntbot Dashboard',
  description: 'Web dashboard for Klyntbot.',
  status: 'Active' as const,
  color: 'orange' as const,
  tags: ['frontend', 'dashboard'],
};

const makeProjects = (count: number) =>
  Array.from({ length: count }, (_, i) => ({
    id: `proj-${i}`,
    name: `Project ${i + 1}`,
    status: 'Active' as const,
    color: 'blue' as const,
  }));

describe('ProjectGrid', () => {
  it('AC-09.1: renders project cards in grid', () => {
    const projects = makeProjects(5);
    render(ProjectGrid, { props: { projects } });
    const listItems = screen.getAllByRole('listitem');
    expect(listItems).toHaveLength(5);
  });

  it('shows empty state when no projects', () => {
    render(ProjectGrid, { props: { projects: [] } });
    expect(screen.getByTestId('empty-state')).toBeInTheDocument();
    expect(screen.getByText(/No projects/i)).toBeInTheDocument();
  });

  it('loading state shows skeleton cards', () => {
    render(ProjectGrid, { props: { projects: [], loading: true } });
    // Skeleton cards have aria-hidden and are inside the busy container
    const busyContainer = document.querySelector('[aria-busy="true"]');
    expect(busyContainer).not.toBeNull();
    const skeletons = busyContainer!.querySelectorAll('.skeleton-card');
    expect(skeletons).toHaveLength(4);
  });
});

describe('ProjectCard', () => {
  it('AC-09.1: renders color stripe, name, description, task count, progress', () => {
    render(ProjectCard, {
      props: {
        project: { ...sampleProject, color: 'orange' },
        taskCount: 8,
        progress: 60,
      },
    });
    expect(screen.getByText('Klyntbot Dashboard')).toBeInTheDocument();
    expect(screen.getByText('Web dashboard for Klyntbot.')).toBeInTheDocument();
    expect(screen.getByText('8 tasks')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '60');
    const stripe = document.querySelector('[data-testid="color-stripe"]') as HTMLElement;
    expect(stripe?.style.backgroundColor).toContain('255');
  });

  it('click fires onSelect', async () => {
    const onSelect = vi.fn();
    render(ProjectCard, {
      props: { project: sampleProject, onSelect },
    });
    const card = screen.getByRole('button', { name: /Klyntbot Dashboard/i });
    await fireEvent.click(card);
    expect(onSelect).toHaveBeenCalledWith(sampleProject);
  });
});

describe('ProjectDetail', () => {
  const tasks = [
    { id: 't1', title: 'Setup CI/CD', status: 'Done', timeEntries: [{ id: 'te1', minutes: 30 }] },
    { id: 't2', title: 'Write tests', status: 'Doing', timeEntries: [] },
    { id: 't3', title: 'Deploy staging', status: 'Todo', timeEntries: [] },
  ];

  it('AC-09.3: renders project header with stats', () => {
    render(ProjectDetail, { props: { project: sampleProject, tasks } });
    const stats = screen.getByTestId('project-stats');
    expect(stats).toBeInTheDocument();
    // 3 total tasks
    expect(stats.textContent).toContain('3');
    // 33% complete (1/3)
    expect(stats.textContent).toContain('33%');
    // 30m tracked
    expect(stats.textContent).toContain('30m');
  });

  it('AC-09.3: renders task tree below header', () => {
    render(ProjectDetail, { props: { project: sampleProject, tasks } });
    const tree = screen.getByTestId('task-tree');
    expect(tree).toBeInTheDocument();
    expect(screen.getByText('Setup CI/CD')).toBeInTheDocument();
    expect(screen.getByText('Write tests')).toBeInTheDocument();
  });

  it('AC-09.4: archive action available', () => {
    render(ProjectDetail, { props: { project: sampleProject, tasks: [] } });
    const archiveBtn = screen.getByRole('button', { name: /Archive/i });
    expect(archiveBtn).toBeInTheDocument();
  });
});

describe('ProjectForm', () => {
  it('AC-09.2: renders name, description, color picker, tags fields', () => {
    render(ProjectForm, {});
    expect(screen.getByLabelText(/Name/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Description/i)).toBeInTheDocument();
    const colorGroup = screen.getByRole('group', { name: /Project color/i });
    expect(colorGroup).toBeInTheDocument();
    expect(screen.getByLabelText(/Tags/i)).toBeInTheDocument();
  });

  it('validates required project name', async () => {
    render(ProjectForm, {});
    const submitBtn = screen.getByRole('button', { name: /Create Project/i });
    await fireEvent.click(submitBtn);
    expect(await screen.findByText('Name is required')).toBeInTheDocument();
  });

  it('color picker limited to 7 enum values', () => {
    render(ProjectForm, {});
    const colorGroup = screen.getByRole('group', { name: /Project color/i });
    const radios = colorGroup.querySelectorAll('input[type="radio"]');
    expect(radios).toHaveLength(7);
    const values = Array.from(radios).map((r) => (r as HTMLInputElement).value);
    expect(values).toEqual(expect.arrayContaining(['red', 'orange', 'yellow', 'green', 'blue', 'purple', 'gray']));
  });
});
