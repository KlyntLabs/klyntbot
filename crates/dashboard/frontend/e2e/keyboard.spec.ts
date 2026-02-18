/**
 * E2E: Keyboard shortcuts and command palette — Flow 20.
 */

import { test, expect } from '@playwright/test';

test.describe('Keyboard Shortcuts', () => {
  test('AC-20.1: Cmd+K opens command palette', async ({ page }) => {
    // Given: Dashboard loaded
    // When: Press Cmd+K (or Ctrl+K)
    // Then: Command palette modal opens with search input focused
    test.fail();
  });

  test('AC-20.2: command palette searches pages and tasks', async ({ page }) => {
    // Given: Palette open
    // When: Type "task"
    // Then: Results show matching pages and tasks
    test.fail();
  });

  test('AC-20.3: Cmd+N opens new task modal', async ({ page }) => {
    // Given: Dashboard loaded
    // When: Press Cmd+N
    // Then: TaskCreateModal opens
    test.fail();
  });
});
