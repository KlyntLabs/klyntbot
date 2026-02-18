/**
 * E2E: Responsive design — CC-2.
 */

import { test, expect } from '@playwright/test';

test.describe('Responsive Design', () => {
  test('CC-2.1: mobile layout (375px)', async ({ page }) => {
    // Given: Viewport 375x812
    // When: Dashboard loads
    // Then: Sidebar hidden, hamburger menu visible, chat hidden
    await page.setViewportSize({ width: 375, height: 812 });
    // TODO: navigate, assert layout
    test.fail();
  });

  test('CC-2.2: tablet layout (768px)', async ({ page }) => {
    // Given: Viewport 768x1024
    // When: Dashboard loads
    // Then: Sidebar collapsed (icons only), chat toggle button in top bar
    await page.setViewportSize({ width: 768, height: 1024 });
    test.fail();
  });

  test('CC-2.3: desktop layout (1280px)', async ({ page }) => {
    // Given: Viewport 1280x800
    // When: Dashboard loads
    // Then: Full sidebar (260px), chat panel docked right (400px)
    await page.setViewportSize({ width: 1280, height: 800 });
    test.fail();
  });
});
