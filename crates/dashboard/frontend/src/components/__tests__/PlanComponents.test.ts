/**
 * Plan component tests — PlanStepper, StepList, StepItem, BacktrackTimeline.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import PlanStepper from '../plans/PlanStepper.svelte';
import StepList from '../plans/StepList.svelte';
import StepItem from '../plans/StepItem.svelte';
import BacktrackTimeline from '../plans/BacktrackTimeline.svelte';

const makePlan = (status: 'Draft' | 'Approved' | 'Executing' | 'Completed' | 'Failed' | 'Abandoned') => ({
  id: 'plan-001',
  title: 'Deploy v2.0',
  status,
});

const sampleSteps = [
  { id: 's1', description: 'Run test suite', status: 'Completed' as const, result: 'All 910 tests passed', attemptCount: 1 },
  { id: 's2', description: 'Build release', status: 'Completed' as const, result: 'Binary 12.3MB', attemptCount: 1 },
  { id: 's3', description: 'Deploy to staging', status: 'Running' as const, result: null, attemptCount: 1 },
  { id: 's4', description: 'Run smoke tests', status: 'Pending' as const, result: null, attemptCount: 0 },
  { id: 's5', description: 'Promote to prod', status: 'Pending' as const, result: null, attemptCount: 0 },
];

describe('PlanStepper', () => {
  it('AC-13.1: shows lifecycle status bar', () => {
    render(PlanStepper, { props: { plan: makePlan('Executing') } });
    expect(screen.getByText('Draft')).toBeInTheDocument();
    expect(screen.getByText('Approved')).toBeInTheDocument();
    expect(screen.getByText('Executing')).toBeInTheDocument();
    expect(screen.getByText('Completed')).toBeInTheDocument();
  });

  it('AC-13.2: shows Abandon button for Executing plan', () => {
    render(PlanStepper, { props: { plan: makePlan('Executing') } });
    expect(screen.getByRole('button', { name: /Abandon/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Approve/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /Execute/i })).toBeNull();
  });

  it('AC-13.2: shows Approve and Abandon for Draft plan', () => {
    render(PlanStepper, { props: { plan: makePlan('Draft') } });
    expect(screen.getByRole('button', { name: /Approve/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Abandon/i })).toBeInTheDocument();
  });

  it('AC-13.2: shows Execute and Abandon for Approved plan', () => {
    render(PlanStepper, { props: { plan: makePlan('Approved') } });
    expect(screen.getByRole('button', { name: /Execute/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Abandon/i })).toBeInTheDocument();
  });

  it('CC-3.1: has progressbar ARIA role', () => {
    render(PlanStepper, { props: { plan: makePlan('Executing') } });
    const bar = screen.getByRole('progressbar');
    expect(bar).toBeInTheDocument();
    expect(bar).toHaveAttribute('aria-valuenow');
    expect(bar).toHaveAttribute('aria-valuemin', '0');
    expect(bar).toHaveAttribute('aria-valuemax', '3');
  });
});

describe('StepList', () => {
  it('AC-14.1: renders all steps with status icons', () => {
    render(StepList, { props: { steps: sampleSteps } });
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(5);
    // Check step descriptions are visible
    expect(screen.getByText('Run test suite')).toBeInTheDocument();
    expect(screen.getByText('Deploy to staging')).toBeInTheDocument();
  });

  it('AC-14.1: current step highlighted with pulse animation', () => {
    render(StepList, { props: { steps: sampleSteps } });
    // The running step should have the 'running' class (which triggers pulse animation)
    const runningStep = document.querySelector('.step-item.running');
    expect(runningStep).not.toBeNull();
  });

  it('completed step shows collapsible result summary', async () => {
    render(StepList, { props: { steps: sampleSteps } });
    // Find the toggle for a completed step with a result
    const toggleBtns = screen.getAllByText(/Result/i);
    expect(toggleBtns.length).toBeGreaterThan(0);
    // Expand the first result
    await fireEvent.click(toggleBtns[0]);
    expect(screen.getByText('All 910 tests passed')).toBeInTheDocument();
  });
});

describe('StepItem', () => {
  it('AC-14.2: failed step shows error and attempt count', () => {
    const failedStep = {
      id: 'sf1',
      description: 'Deploy to staging',
      status: 'Failed' as const,
      errorMessage: 'Connection timeout after 30s',
      attemptCount: 2,
      maxAttempts: 3,
    };
    render(StepItem, { props: { step: failedStep, index: 0 } });
    expect(screen.getByText('Connection timeout after 30s')).toBeInTheDocument();
    expect(screen.getByText(/Attempt 2\/3/i)).toBeInTheDocument();
  });
});

describe('BacktrackTimeline', () => {
  const entries = [
    {
      id: 'bt1',
      stepIndex: 2,
      stepDescription: 'Deploy to staging',
      reason: 'Connection timeout',
      regeneratedSteps: ['Retry deploy with increased timeout'],
      timestamp: '2026-02-18T10:30:00Z',
    },
  ];

  it('AC-14.3: renders backtrack entries', async () => {
    render(BacktrackTimeline, { props: { entries } });
    // Initially collapsed — expand it
    const header = screen.getByRole('button', { name: /Backtrack History/i });
    await fireEvent.click(header);
    expect(screen.getByText('Connection timeout')).toBeInTheDocument();
    expect(screen.getByText(/Retry deploy/i)).toBeInTheDocument();
  });

  it('collapsible by default', () => {
    render(BacktrackTimeline, { props: { entries } });
    // Header shows event count
    expect(screen.getByText(/1 event/i)).toBeInTheDocument();
    // Entries are NOT visible initially
    expect(screen.queryByText('Connection timeout')).toBeNull();
    // aria-expanded should be false
    const header = screen.getByRole('button');
    expect(header).toHaveAttribute('aria-expanded', 'false');
  });
});
