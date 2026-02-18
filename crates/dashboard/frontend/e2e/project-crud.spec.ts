/**
 * E2E: Project management — Flow 9.
 */

import { test, expect } from '@playwright/test';

test.describe('Project CRUD', () => {
  test('AC-09.1: project grid displays cards', async ({ page }) => {
    // Given: Seeded projects
    // When: Navigate to Projects
    // Then: Project cards visible with color, name, task count, progress
    test.fail();
  });

  test('AC-09.2: create project via modal', async ({ page }) => {
    // Given: Projects page
    // When: Click "+ New Project", fill name, select color, click Create
    // Then: New project card appears in grid
    test.fail();
  });

  test('AC-09.3: project detail with task tree', async ({ page }) => {
    // Given: Project with tasks
    // When: Click project card
    // Then: Detail page shows stats header and task tree
    test.fail();
  });

  test('AC-09.4: archive project', async ({ page }) => {
    // Given: Project detail page
    // When: Click Archive button, confirm
    // Then: Project status changes to Archived
    test.fail();
  });
});
