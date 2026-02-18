/**
 * RRuleBuilder component tests.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import RRuleBuilder from '../misc/RRuleBuilder.svelte';

describe('RRuleBuilder', () => {
  it('AC-08.1: renders frequency, interval, day selection, end condition', () => {
    render(RRuleBuilder, { props: {} });
    expect(screen.getByLabelText('Frequency')).toBeTruthy();
    expect(screen.getByLabelText('Every')).toBeTruthy();
    // Weekly selected by default — day buttons visible
    expect(screen.getByText('Mon')).toBeTruthy();
    expect(screen.getByLabelText('Ends')).toBeTruthy();
  });

  it('AC-08.2: generates preview text', async () => {
    const { container } = render(RRuleBuilder, { props: {} });
    // Change to weekly, interval 2
    const freqSelect = screen.getByLabelText('Frequency') as HTMLSelectElement;
    await fireEvent.change(freqSelect, { target: { value: 'WEEKLY' } });
    const intervalInput = screen.getByLabelText('Every') as HTMLInputElement;
    await fireEvent.input(intervalInput, { target: { value: '2' } });
    // Select Mon and Wed
    await fireEvent.click(screen.getByText('Mon'));
    await fireEvent.click(screen.getByText('Wed'));
    const preview = container.querySelector('.preview-text');
    expect(preview?.textContent).toContain('Every 2 week');
    expect(preview?.textContent).toContain('Mon');
    expect(preview?.textContent).toContain('Wed');
  });

  it('day selector visible only for weekly frequency', async () => {
    render(RRuleBuilder, { props: {} });
    // Default is weekly — days should be visible
    expect(screen.getByText('Mon')).toBeTruthy();
    // Change to Daily
    const freqSelect = screen.getByLabelText('Frequency') as HTMLSelectElement;
    await fireEvent.change(freqSelect, { target: { value: 'DAILY' } });
    // Day buttons should be hidden
    expect(screen.queryByText('Mon')).toBeNull();
  });

  it('fires onChange with RRULE string', async () => {
    const onChange = vi.fn();
    render(RRuleBuilder, { props: { onChange } });
    // onChange fires reactively
    expect(onChange).toHaveBeenCalled();
    const [firstCall] = onChange.mock.calls;
    expect(firstCall[0]).toMatch(/^RRULE:FREQ=/);
  });
});
