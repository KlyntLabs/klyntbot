/**
 * E2E: Settings management — Flow 16, CC-6.
 */

import { test, expect } from '@playwright/test';

test.describe('Settings', () => {
  test('AC-16.1: settings page loads with tabs', async ({ page }) => {
    // Given: Dashboard running
    // When: Navigate to Settings
    // Then: All tabs visible (Providers, Channels, Tools, etc.)
    test.fail();
  });

  test('AC-16.2: API keys masked by default', async ({ page }) => {
    // Given: Settings > Providers tab
    // When: Page loads
    // Then: API key fields show masked values
    test.fail();
  });

  test('AC-16.4: config update saves and hot-reloads', async ({ page }) => {
    // Given: Settings page
    // When: Change enrichment threshold slider, click Save
    // Then: Config file updated, hot-reload indicator flashes
    test.fail();
  });

  test('CC-6.1: external config change reflects in UI', async ({ page }) => {
    // Given: Settings page open
    // When: Config file is modified externally
    // Then: Settings form refreshes with new values
    test.fail();
  });
});
