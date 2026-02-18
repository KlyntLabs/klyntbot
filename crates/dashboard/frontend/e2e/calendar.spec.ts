/**
 * E2E: Calendar view and sync — Flows 11, 12.
 */

import { test, expect } from '@playwright/test';

test.describe('Calendar', () => {
  test('AC-11.1: month view with events', async ({ page }) => {
    // Given: Calendar events seeded
    // When: Navigate to Calendar
    // Then: Month view with day cells and event bars
    test.fail();
  });

  test('AC-11.1: switch between month/week/day views', async ({ page }) => {
    // Given: Calendar page
    // When: Click Week view toggle
    // Then: Time grid layout renders
    test.fail();
  });

  test('AC-12.1: sync controls show status', async ({ page }) => {
    // Given: Calendar page
    // When: Check sync controls
    // Then: Last sync time displayed
    test.fail();
  });

  test('AC-12.2: manual sync trigger', async ({ page }) => {
    // Given: Calendar page
    // When: Click "Sync Now"
    // Then: Spinner shows during sync, sync result displayed
    test.fail();
  });
});
