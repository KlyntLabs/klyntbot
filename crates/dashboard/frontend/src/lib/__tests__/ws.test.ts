/**
 * Tests for the WebSocket client (lib/ws.ts — AgentSocket class).
 *
 * Covers:
 * - AC-17.2: AgentSocket connects to /ws
 * - AC-17.2: Auto-reconnects after 2s on disconnect
 * - Message sending (chat.send, interaction.respond, chat.cancel)
 * - Incoming message parsing and event emission
 * - Status: connected / reconnecting / disconnected
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { AgentSocket } from '../ws';
import { MockWebSocket } from '../../__tests__/setup';

describe('AgentSocket', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers(); // For reconnection delay testing
  });

  // ── Connection ────────────────────────────────────────────────────────────

  it('connects to /ws endpoint', () => {
    // AC-17.2: new AgentSocket() connects to ws://localhost/ws (or relative path)
    const socket = new AgentSocket();
    expect(MockWebSocket.instances[0].url).toContain('/ws');
    socket.disconnect();
  });

  it('reports status as "connected" after open', () => {
    // AC-17.2: After WebSocket onopen fires, status is "connected"
    const socket = new AgentSocket();
    expect(socket.getStatus()).toBe('connecting');
    MockWebSocket.instances[0].simulateOpen();
    expect(socket.getStatus()).toBe('connected');
    socket.disconnect();
  });

  it('reports status as "connecting" initially (before open)', () => {
    // Before connection opens, status is "connecting"
    const socket = new AgentSocket();
    expect(socket.getStatus()).toBe('connecting');
    socket.disconnect();
  });

  // ── Auto-reconnect ────────────────────────────────────────────────────────

  it('reports status as "reconnecting" after disconnect', () => {
    // AC-17.2, UX §1.4: After onclose fires, status transitions to "reconnecting"
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    MockWebSocket.instances[0].close();
    expect(socket.getStatus()).toBe('reconnecting');
    vi.runAllTimers();
    socket.disconnect();
  });

  it('reconnects after 2 seconds following disconnect', () => {
    // AC-17.2, UX §1.4: Reconnection attempt fires after 2s delay
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    MockWebSocket.instances[0].close();
    expect(MockWebSocket.instances.length).toBe(1); // Not yet reconnected
    vi.advanceTimersByTime(2000);
    expect(MockWebSocket.instances.length).toBe(2); // New connection attempt
    socket.disconnect();
  });

  it('reports status as "connected" after successful reconnect', () => {
    // AC-17.2: After successful reconnect, status returns to "connected"
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    MockWebSocket.instances[0].close();
    vi.advanceTimersByTime(2000);
    MockWebSocket.instances[1].simulateOpen();
    expect(socket.getStatus()).toBe('connected');
    socket.disconnect();
  });

  // ── Message sending ───────────────────────────────────────────────────────

  it('sendChatMessage sends chat.send message with session key', () => {
    // AC-17.2: sendChatMessage() sends { type: "chat.send", sessionKey, message }
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    socket.sendChatMessage('my-session', 'hello');
    const sent = JSON.parse(MockWebSocket.instances[0].sentMessages[0]) as Record<string, unknown>;
    expect(sent.type).toBe('chat.send');
    expect(sent.sessionKey).toBe('my-session');
    expect(sent.message).toBe('hello');
    socket.disconnect();
  });

  it('sendCancel sends chat.cancel message', () => {
    // AC-WS.4: cancel() sends { type: "chat.cancel" }
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    socket.sendCancel();
    const sent = JSON.parse(MockWebSocket.instances[0].sentMessages[0]) as Record<string, unknown>;
    expect(sent.type).toBe('chat.cancel');
    socket.disconnect();
  });

  it('sendInteractionResponse sends interaction.respond with requestId', () => {
    // AC-WS.5: respondToInteraction(requestId, response) sends correct message shape
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    socket.sendInteractionResponse('req-123', { Completed: ['yes'] });
    const sent = JSON.parse(MockWebSocket.instances[0].sentMessages[0]) as Record<string, unknown>;
    expect(sent.type).toBe('interaction.respond');
    expect(sent.requestId).toBe('req-123');
    socket.disconnect();
  });

  it('queues messages sent before connection is open', () => {
    // Edge: If send() is called before onopen, messages are buffered and sent on open
    const socket = new AgentSocket();
    // Don't call simulateOpen yet — send while still connecting
    socket.sendChatMessage(undefined, 'queued message');
    // Message should not be in sentMessages yet (WS not open)
    expect(MockWebSocket.instances[0].sentMessages).toHaveLength(0);
    // Now open the connection — pending messages should flush
    MockWebSocket.instances[0].simulateOpen();
    expect(MockWebSocket.instances[0].sentMessages).toHaveLength(1);
    socket.disconnect();
  });

  // ── Incoming message parsing ──────────────────────────────────────────────

  it('calls onMessage callback with parsed event for contentChunk', () => {
    // AC-17.2: Incoming {"type":"contentChunk","data":"hello"} is parsed and
    // delivered to registered message handler
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    const handler = vi.fn();
    socket.onMessage(handler);
    MockWebSocket.instances[0].simulateMessage({ type: 'contentChunk', data: 'hello' });
    expect(handler).toHaveBeenCalledWith({ type: 'contentChunk', data: 'hello' });
    socket.disconnect();
  });

  it('calls onMessage callback with parsed event for done', () => {
    // AC-17.3: done event is forwarded to message handler
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    const handler = vi.fn();
    socket.onMessage(handler);
    MockWebSocket.instances[0].simulateMessage({ type: 'done', content: 'final answer' });
    expect(handler).toHaveBeenCalledWith({ type: 'done', content: 'final answer' });
    socket.disconnect();
  });

  it('handles malformed JSON without crashing', () => {
    // AC-17.2: Receiving non-JSON text does not throw; logs error
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    // Directly trigger onmessage with non-JSON
    MockWebSocket.instances[0].onmessage?.({
      data: 'not valid json {{{{',
    } as MessageEvent);
    expect(consoleErrorSpy).toHaveBeenCalled();
    socket.disconnect();
    consoleErrorSpy.mockRestore();
  });

  // ── Cleanup ───────────────────────────────────────────────────────────────

  it('disconnect() closes the WebSocket and stops reconnection', () => {
    // Edge case: explicit disconnect should not trigger reconnect loop
    const socket = new AgentSocket();
    MockWebSocket.instances[0].simulateOpen();
    socket.disconnect();
    expect(socket.getStatus()).toBe('disconnected');
    // After explicit disconnect, no reconnection should happen
    vi.advanceTimersByTime(5000);
    expect(MockWebSocket.instances.length).toBe(1); // No new connections
  });
});
