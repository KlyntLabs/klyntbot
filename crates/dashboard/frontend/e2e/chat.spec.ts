/**
 * E2E: Chat interaction — Flow 15.
 */

import { test, expect } from '@playwright/test';

test.describe('Chat', () => {
  test('AC-15.1: send message and receive response', async ({ page }) => {
    // Given: Dashboard with chat panel open
    // When: Type "Show me my tasks" and press Enter
    // Then: User bubble appears, streaming response begins, tool calls shown
    test.fail();
  });

  test('AC-15.2: streaming response renders token-by-token', async ({ page }) => {
    // Given: Message sent
    // When: Agent responds
    // Then: Text appears incrementally, typing indicator shown during stream
    test.fail();
  });

  test('AC-15.3: tool call cards display', async ({ page }) => {
    // Given: Agent uses todo tool
    // When: Tool executes
    // Then: ToolCallCard appears with tool name, status, result
    test.fail();
  });

  test('AC-15.4: create new chat session', async ({ page }) => {
    // Given: Existing chat session
    // When: Click "+" new session button
    // Then: New empty session created, old session in sidebar list
    test.fail();
  });
});
