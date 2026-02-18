/**
 * E2E: Dashboard overview — Flows 1, 18.
 */

import { test, expect } from '@playwright/test';

test.describe('Dashboard Overview', () => {
  test('AC-01.1: loads with summary cards', async ({ page }) => {
    // Given: klyntbot serve running with seeded data
    // When: Navigate to http://localhost:8080
    // Then: Summary cards (active tasks, in focus, events today) visible with counts
    // TODO: await page.goto('http://localhost:8080');
    // TODO: expect summary cards
    test.fail();
  });

  test('AC-01.2: recent activity feed shows entries', async ({ page }) => {
    // Given: Recent actions in store
    // When: Dashboard loads
    // Then: Activity feed shows entries with timestamps
    test.fail();
  });

  test('AC-18.1: system status cards display', async ({ page }) => {
    // Given: Agent running
    // When: Dashboard loads
    // Then: System status section shows agent, channels, calendar status
    test.fail();
  });
});
