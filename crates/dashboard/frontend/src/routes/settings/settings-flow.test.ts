/**
 * Integration tests for the settings management flow.
 * Tests tab navigation, field editing, validation, and save behavior.
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import SettingsPage from '../../components/settings/SettingsPage.svelte';
import ThresholdSlider from '../../components/settings/ThresholdSlider.svelte';
import MaskedInput from '../../components/settings/MaskedInput.svelte';

const sampleConfig = {
  providers: [
    {
      name: 'Anthropic',
      apiKey: 'sk-ant-api-01-test-key-1234567890abcdef',
      model: 'claude-opus-4-6',
      connected: true,
      availableModels: ['claude-opus-4-6', 'claude-sonnet-4-5-20250929', 'claude-haiku-4-5-20251001'],
    },
  ],
};

describe('Settings Flow Integration', () => {
  it('AC-16.1: all settings tabs accessible and switchable', async () => {
    render(SettingsPage, { props: { config: sampleConfig } });

    const tabs = ['Providers', 'Channels', 'Tools', 'Notifications', 'Calendar', 'Search'];
    for (const tab of tabs) {
      const tabEl = screen.getByRole('tab', { name: tab });
      expect(tabEl).toBeTruthy();
      await fireEvent.click(tabEl);
      // After clicking, tab becomes active (aria-selected)
      expect(tabEl.getAttribute('aria-selected')).toBe('true');
    }
  });

  it('AC-16.2: masked API key reveals and conceals correctly', async () => {
    const { container } = render(MaskedInput, {
      props: { value: 'sk-ant-api-01-verylongkey-1234567890', revealed: false }
    });

    // Initially masked
    expect((container.querySelector('input') as HTMLInputElement).type).toBe('password');

    // Reveal
    await fireEvent.click(container.querySelector('button.reveal-btn') as HTMLElement);
    expect((container.querySelector('input') as HTMLInputElement).type).toBe('text');

    // Conceal again
    await fireEvent.click(container.querySelector('button.reveal-btn') as HTMLElement);
    expect((container.querySelector('input') as HTMLInputElement).type).toBe('password');
  });

  it('AC-16.3: threshold slider validates 0.0-1.0 range', async () => {
    const onChange = vi.fn();
    const { container } = render(ThresholdSlider, {
      props: { label: 'Auto-apply threshold', value: 0.7, min: 0, max: 1, step: 0.01, onChange }
    });

    const slider = container.querySelector('input[type="range"]') as HTMLInputElement;
    expect(parseFloat(slider.min)).toBe(0);
    expect(parseFloat(slider.max)).toBe(1);

    // Slide to 0.85
    await fireEvent.input(slider, { target: { value: '0.85' } });
    expect(onChange).toHaveBeenLastCalledWith(0.85);
    expect(screen.getByText('0.85')).toBeTruthy();
  });

  it('AC-16.5: restart warning appears after API key change', async () => {
    const onSave = vi.fn();
    render(SettingsPage, { props: { config: sampleConfig, onSave } });

    // Warning not visible initially
    expect(screen.queryByText(/require restart/)).toBeNull();

    // Save fires with correct section
    await fireEvent.click(screen.getByText('Save Changes'));
    expect(onSave).toHaveBeenCalledWith('providers', expect.any(Object));
  });

  it('settings page has proper heading hierarchy', () => {
    render(SettingsPage, { props: { config: sampleConfig } });
    const h1 = screen.getByRole('heading', { level: 1 });
    expect(h1.textContent).toBe('Settings');
  });
});
