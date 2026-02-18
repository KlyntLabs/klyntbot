/**
 * GoalListPage component tests.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import GoalListPage from '../goals/GoalListPage.svelte';

const makeGoal = (id: string, progress: number) => ({
  id,
  title: `Goal ${id}`,
  description: `Description for goal ${id}`,
  status: 'Active' as const,
  metrics: [{ name: 'Tasks', current: progress, target: 100, unit: '%' }],
});

const sampleGoals = [
  makeGoal('g1', 25),
  makeGoal('g2', 50),
  makeGoal('g3', 75),
  makeGoal('g4', 100),
];

describe('GoalListPage', () => {
  it('AC-10.1: renders goal cards with progress bars', () => {
    render(GoalListPage, { props: { goals: sampleGoals } });
    // 4 goal cards in the list
    const listItems = screen.getAllByRole('listitem');
    expect(listItems).toHaveLength(4);
    // Each card has a progress bar
    const progressBars = screen.getAllByRole('progressbar');
    expect(progressBars).toHaveLength(4);
  });

  it('AC-10.2: new goal button opens create modal', async () => {
    const onNew = vi.fn();
    render(GoalListPage, { props: { goals: sampleGoals, onNew } });
    const newBtn = screen.getByRole('button', { name: /\+ New Goal/i });
    await fireEvent.click(newBtn);
    expect(onNew).toHaveBeenCalledOnce();
  });

  it('goal card click navigates to detail', async () => {
    const onSelect = vi.fn();
    render(GoalListPage, { props: { goals: sampleGoals, onSelect } });
    const firstCard = screen.getAllByRole('listitem')[0];
    await fireEvent.click(firstCard);
    expect(onSelect).toHaveBeenCalledWith(sampleGoals[0]);
  });

  it('shows empty state when no goals', () => {
    render(GoalListPage, { props: { goals: [] } });
    expect(screen.getByTestId('empty-state')).toBeInTheDocument();
    expect(screen.getByText(/No goals yet/i)).toBeInTheDocument();
  });
});
