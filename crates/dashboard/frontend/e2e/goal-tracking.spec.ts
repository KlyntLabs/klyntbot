/**
 * E2E: Goal tracking — Flow 10.
 */

import { test, expect } from '@playwright/test';

test.describe('Goal Tracking', () => {
  test('AC-10.1: goal cards with progress bars', async ({ page }) => {
    // Given: Seeded goals with metrics
    // When: Navigate to Goals
    // Then: Goal cards visible with progress bars based on metrics
    test.fail();
  });

  test('AC-10.2: create goal', async ({ page }) => {
    // Given: Goals page
    // When: Click "+ New Goal", fill title and target metric
    // Then: New goal card appears
    test.fail();
  });

  test('AC-10.3: update metric updates progress', async ({ page }) => {
    // Given: Goal detail open
    // When: Update metric current value
    // Then: Progress bar animates to new percentage
    test.fail();
  });
});
