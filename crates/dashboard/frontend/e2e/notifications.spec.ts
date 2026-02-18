/**
 * E2E: Notification management — Flow 17.
 */

import { test, expect } from '@playwright/test';

test.describe('Notifications', () => {
  test('AC-17.1: notification bell shows unread count', async ({ page }) => {
    // Given: 3 unread notifications
    // When: Dashboard loads
    // Then: Bell icon shows badge "3"
    test.fail();
  });

  test('AC-17.2: notification drawer opens and lists items', async ({ page }) => {
    // Given: Notifications exist
    // When: Click notification bell
    // Then: Drawer slides in with grouped notifications
    test.fail();
  });

  test('AC-17.3: click notification navigates to source', async ({ page }) => {
    // Given: Notification about overdue task
    // When: Click notification item
    // Then: Navigates to task detail
    test.fail();
  });
});
