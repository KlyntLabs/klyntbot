/**
 * E2E: Task search — Flow 4.
 */

import { test, expect } from '@playwright/test';

test.describe('Task Search', () => {
  test('AC-04.1: keyword search filters tasks', async ({ page }) => {
    // Given: Tasks page with seeded data
    // When: Type "auth" in search bar
    // Then: Only tasks matching "auth" shown
    test.fail();
  });

  test('AC-04.2: semantic search with results', async ({ page }) => {
    // Given: Semantic search mode selected
    // When: Search "login problems"
    // Then: Returns related tasks (e.g., "Fix authentication regression") with similarity scores
    test.fail();
  });

  test('AC-04.4: clear search restores full view', async ({ page }) => {
    // Given: Active search filtering tasks
    // When: Click "Clear search"
    // Then: All tasks visible again
    test.fail();
  });
});
