/**
 * EnrichmentModal component tests — AI task enrichment suggestions.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import EnrichmentModal from '../tasks/EnrichmentModal.svelte';

const mockSuggestions = [
  { field: 'priority', value: 'High (2)', confidence: 0.85 },
  { field: 'duration', value: '30 min', confidence: 0.72 },
  { field: 'dueDate', value: 'Today', confidence: 0.90 },
];

describe('EnrichmentModal', () => {
  it('AC-19.1: displays suggestions with confidence scores', () => {
    render(EnrichmentModal, { props: { suggestions: mockSuggestions } });
    expect(screen.getByTestId('suggestion-priority')).toBeDefined();
    expect(screen.getByTestId('suggestion-duration')).toBeDefined();
    expect(screen.getByTestId('suggestion-dueDate')).toBeDefined();
    expect(screen.getByTestId('confidence-bar-priority')).toBeDefined();
    const bar = screen.getByTestId('confidence-bar-priority');
    expect(bar.style.width).toBe('85%');
  });

  it('AC-19.2: accept individual suggestion', async () => {
    render(EnrichmentModal, { props: { suggestions: mockSuggestions } });
    await fireEvent.click(screen.getByTestId('accept-priority'));
    const row = screen.getByTestId('suggestion-priority');
    expect(row.classList.contains('accepted')).toBe(true);
  });

  it('AC-19.2: reject individual suggestion', async () => {
    render(EnrichmentModal, { props: { suggestions: mockSuggestions } });
    await fireEvent.click(screen.getByTestId('reject-duration'));
    const row = screen.getByTestId('suggestion-duration');
    expect(row.classList.contains('rejected')).toBe(true);
  });

  it('apply selected fires onApply with accepted suggestions', async () => {
    const onApply = vi.fn();
    render(EnrichmentModal, { props: { suggestions: mockSuggestions, onApply } });
    await fireEvent.click(screen.getByTestId('accept-priority'));
    await fireEvent.click(screen.getByTestId('accept-dueDate'));
    await fireEvent.click(screen.getByTestId('reject-duration'));
    await fireEvent.click(screen.getByTestId('apply-btn'));
    expect(onApply).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({ field: 'priority' }),
        expect.objectContaining({ field: 'dueDate' }),
      ])
    );
    const calls = onApply.mock.calls[0][0];
    expect(calls).toHaveLength(2);
  });
});
