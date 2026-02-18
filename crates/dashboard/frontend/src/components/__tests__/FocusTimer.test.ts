/**
 * FocusTimer component tests — focus session widget.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import FocusTimer from '../tasks/FocusTimer.svelte';

describe('FocusTimer', () => {
  it('AC-05.1: starts timer on mount with running=true', () => {
    render(FocusTimer, { props: { taskId: 't1', running: true, elapsed: 0 } });
    expect(screen.getByTestId('timer-display').textContent?.trim()).toBe('00:00');
  });

  it('AC-05.3: shows Pause button when running', () => {
    render(FocusTimer, { props: { taskId: 't1', running: true } });
    expect(screen.getByTestId('pause-btn')).toBeDefined();
    expect(screen.getByTestId('complete-btn')).toBeDefined();
  });

  it('AC-05.3: Pause stops timer, shows Resume', async () => {
    render(FocusTimer, { props: { taskId: 't1', running: true } });
    await fireEvent.click(screen.getByTestId('pause-btn'));
    expect(screen.getByTestId('resume-btn')).toBeDefined();
    expect(screen.queryByTestId('pause-btn')).toBeNull();
  });

  it('AC-05.3: Resume continues timer', async () => {
    render(FocusTimer, { props: { taskId: 't1', running: false } });
    expect(screen.getByTestId('resume-btn')).toBeDefined();
    await fireEvent.click(screen.getByTestId('resume-btn'));
    expect(screen.getByTestId('pause-btn')).toBeDefined();
  });

  it('AC-05.5: Complete prompts to mark task done', async () => {
    render(FocusTimer, { props: { taskId: 't1', running: true } });
    await fireEvent.click(screen.getByTestId('complete-btn'));
    expect(screen.getByTestId('complete-prompt')).toBeDefined();
    expect(screen.getByTestId('yes-btn')).toBeDefined();
    expect(screen.getByTestId('no-btn')).toBeDefined();
  });

  it('CC-3.1: timer has ARIA role and label', () => {
    render(FocusTimer, { props: { taskId: 't1', running: true, elapsed: 0 } });
    const timer = screen.getByTestId('focus-timer');
    expect(timer.getAttribute('role')).toBe('timer');
    expect(timer.getAttribute('aria-label')).toMatch(/focus timer/i);
  });
});
