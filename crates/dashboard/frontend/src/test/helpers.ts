/**
 * Shared test utilities for Svelte component tests.
 *
 * Provides helpers for rendering with providers, mock GraphQL clients,
 * mock WebSocket connections, and fixture loading.
 */

// TODO: Uncomment when frontend scaffolding is in place
//
// import { render, type RenderResult } from '@testing-library/svelte';
// import type { SvelteComponent } from 'svelte';
//
// /**
//  * Render a component with GraphQL and store context providers.
//  */
// export function renderWithProviders<T extends SvelteComponent>(
//   Component: new (...args: any[]) => T,
//   props: Record<string, any> = {},
// ): RenderResult<T> {
//   // TODO: Wrap with ApolloProvider, store context, WebSocket context
//   return render(Component, { props });
// }
//
// /**
//  * Create a mock GraphQL client backed by MSW handlers.
//  */
// export function createMockGraphQLClient() {
//   // TODO: Set up MSW with default handlers for all queries/mutations
//   return {
//     query: async (doc: any, variables?: any) => ({ data: {} }),
//     mutate: async (doc: any, variables?: any) => ({ data: {} }),
//   };
// }
//
// /**
//  * Create a mock WebSocket for chat and subscription testing.
//  */
// export function createMockWebSocket() {
//   const messages: any[] = [];
//   const listeners: Record<string, Function[]> = {};
//
//   return {
//     send(data: string) { messages.push(JSON.parse(data)); },
//     addEventListener(event: string, fn: Function) {
//       (listeners[event] ??= []).push(fn);
//     },
//     removeEventListener(event: string, fn: Function) {
//       listeners[event] = (listeners[event] ?? []).filter(f => f !== fn);
//     },
//     // Test helper: simulate server message
//     simulateMessage(data: any) {
//       (listeners['message'] ?? []).forEach(fn =>
//         fn({ data: JSON.stringify(data) })
//       );
//     },
//     // Test helper: simulate connection close
//     simulateClose(code = 1000) {
//       (listeners['close'] ?? []).forEach(fn => fn({ code }));
//     },
//     get sentMessages() { return messages; },
//     close() {},
//     readyState: 1, // OPEN
//   };
// }
//
// /**
//  * Load a JSON fixture file.
//  */
// export async function loadFixture<T>(name: string): Promise<T> {
//   const response = await fetch(`/tests/fixtures/dashboard/${name}`);
//   return response.json();
// }
//
// /**
//  * Sample task data for component tests.
//  */
// export const sampleTask = {
//   id: 'todo-001',
//   title: 'Fix authentication regression',
//   description: 'Users unable to login after session token update.',
//   status: 'Doing' as const,
//   priority: 1,
//   tags: ['bug', 'auth', 'urgent'],
//   dueDate: '2026-02-20T00:00:00Z',
//   parentId: null,
//   projectId: 'proj-001',
//   attachments: [
//     { id: 'att-001', type: 'file', title: 'error_screenshot.png', path: '/tmp/error.png' },
//   ],
//   timeEntries: [
//     { id: 'te-001', minutes: 25, note: 'Initial investigation', createdAt: '2026-02-18T09:00:00Z' },
//   ],
//   dependencies: [],
//   focusDeadline: '2026-02-18T17:00:00Z',
// };
//
// export const sampleProject = {
//   id: 'proj-001',
//   name: 'Klyntbot Dashboard',
//   description: 'Web dashboard for Klyntbot.',
//   status: 'Active' as const,
//   color: 'orange',
//   tags: ['frontend', 'dashboard'],
// };
//
// export const samplePlan = {
//   id: 'plan-001',
//   title: 'Deploy Dashboard v1',
//   status: 'Executing' as const,
//   steps: [
//     { id: 'step-001', description: 'Run test suite', status: 'Completed', result: 'All passed', attemptCount: 1 },
//     { id: 'step-002', description: 'Build release', status: 'Completed', result: 'Binary 12.3MB', attemptCount: 1 },
//     { id: 'step-003', description: 'Deploy to staging', status: 'Running', result: null, attemptCount: 1 },
//     { id: 'step-004', description: 'Run smoke tests', status: 'Pending', result: null, attemptCount: 0 },
//   ],
//   backtrackHistory: [],
// };
