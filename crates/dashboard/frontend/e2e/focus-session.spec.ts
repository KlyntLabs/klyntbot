/**
 * E2E: Focus session — Flow 5.
 */

import { test, expect } from '@playwright/test';

test.describe('Focus Session', () => {
  test('AC-05.1: start focus session', async ({ page }) => {
    // Given: Task card visible
    // When: Click "Focus" button
    // Then: Timer widget appears, timer counting up, task status changes to Doing
    test.fail();
  });

  test('AC-05.3: pause and resume timer', async ({ page }) => {
    // Given: Focus session active
    // When: Click Pause
    // Then: Timer stops, Resume button appears
    // When: Click Resume
    // Then: Timer continues
    test.fail();
  });

  test('AC-05.4: complete focus session logs time', async ({ page }) => {
    // Given: Focus session running for ~1 minute
    // When: Click Complete
    // Then: Time entry logged, prompt to mark task done
    test.fail();
  });
});
