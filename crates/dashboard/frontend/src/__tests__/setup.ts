/**
 * Vitest + React Testing Library global setup.
 *
 * This file is referenced in vite.config.ts under test.setupFiles.
 * It runs before every test file.
 *
 * Responsibilities:
 * - Import @testing-library/jest-dom matchers (toBeInTheDocument, etc.)
 * - Set up global fetch mock infrastructure
 * - Set up global WebSocket mock infrastructure
 * - Clean up mocks after each test
 */

import '@testing-library/jest-dom';
import { vi, afterEach, beforeEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// Auto-cleanup after each test (unmounts React components)
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

// Mock WebSocket globally so tests can control connection behavior
// The MockWebSocket class simulates:
//   - connection lifecycle (onopen, onclose, onerror)
//   - message sending/receiving
//   - reconnection (manual trigger via test helpers)
class MockWebSocket {
  // Static readyState constants (same values as native WebSocket)
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  static instances: MockWebSocket[] = [];
  url: string;
  onopen: ((ev: Event) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  readyState = 0; // CONNECTING — use literal to avoid circular ref
  sentMessages: string[] = [];

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  send(data: string) {
    this.sentMessages.push(data);
  }

  close() {
    this.readyState = 3; // CLOSED
    this.onclose?.({ code: 1000, reason: '', wasClean: true } as CloseEvent);
  }

  // Test helper: simulate server sending a message to the client
  simulateMessage(data: unknown) {
    this.onmessage?.({ data: JSON.stringify(data) } as MessageEvent);
  }

  // Test helper: simulate successful connection
  simulateOpen() {
    this.readyState = 1; // OPEN
    this.onopen?.(new Event('open'));
  }

  // Test helper: simulate connection error
  simulateError() {
    this.onerror?.(new Event('error'));
  }
}

// Reset instances before each test so tests start with a clean slate
beforeEach(() => {
  MockWebSocket.instances = [];
});

vi.stubGlobal('WebSocket', MockWebSocket);

// Suppress React Router v7 + jsdom AbortSignal compatibility error.
// When <Navigate> triggers internal navigation in createMemoryRouter, React Router
// creates a new Request() with a jsdom AbortSignal. Undici (Node.js's built-in
// fetch layer) rejects it because its AbortSignal class differs from jsdom's.
// This is a known framework compatibility issue that does not affect test correctness.
const originalUnhandledRejection = process.listeners('unhandledRejection');
void originalUnhandledRejection; // suppress unused warning
process.on('unhandledRejection', (reason: unknown) => {
  const msg = String(reason);
  if (
    msg.includes('AbortSignal') &&
    (msg.includes('instance of AbortSignal') || msg.includes('Expected signal'))
  ) {
    return; // Suppress this known jsdom ↔ React Router v7 compatibility issue
  }
  // Re-throw everything else so real errors surface
  throw reason as Error;
});

export { MockWebSocket };
