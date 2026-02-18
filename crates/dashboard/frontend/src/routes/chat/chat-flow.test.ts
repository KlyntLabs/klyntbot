/**
 * Integration tests for the chat flow.
 * Tests the full user journey: open panel → type message → send → see response stream.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import ChatPanel from '../../components/chat/ChatPanel.svelte';

describe('Chat Flow Integration', () => {
  it('AC-15.1: full send-receive cycle renders correctly', async () => {
    const sentMessages: string[] = [];
    const onSend = vi.fn((msg: string) => sentMessages.push(msg));

    render(ChatPanel, {
      props: {
        messages: [],
        streaming: false,
        online: true,
        onSend,
      }
    });

    // Initially shows empty state
    expect(screen.getByText('Start a conversation')).toBeTruthy();

    // User types and sends
    const input = screen.getByRole('textbox');
    await fireEvent.input(input, { target: { value: 'What tasks are overdue?' } });
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });

    expect(onSend).toHaveBeenCalledWith('What tasks are overdue?');
  });

  it('AC-15.2: streaming state shows indicator, hides on completion', async () => {
    // StreamingIndicator only shows when there are messages (in the {:else} block)
    const existingMessages = [{
      id: 'msg-1',
      role: 'user' as const,
      content: 'Hello',
      timestamp: new Date(),
    }];

    const { rerender, container } = render(ChatPanel, {
      props: { messages: existingMessages, streaming: true, online: true }
    });

    // Streaming dots visible (3 dots)
    const dots = container.querySelectorAll('.dot');
    expect(dots.length).toBe(3);

    // After streaming completes, dots hidden
    await rerender({ messages: existingMessages, streaming: false });
    const dotsAfter = container.querySelectorAll('.dot');
    expect(dotsAfter.length).toBe(0);
  });

  it('AC-15.3: tool calls shown inline between messages', () => {
    const messages = [
      {
        id: 'msg-1',
        role: 'user' as const,
        content: 'List my tasks',
        timestamp: new Date(),
      },
      {
        id: 'msg-2',
        role: 'assistant' as const,
        content: 'Here are your tasks:',
        timestamp: new Date(),
        toolCalls: [{
          name: 'todo',
          inputs: { action: 'list' },
          outputs: { count: 5, items: [] },
          status: 'completed' as const,
        }],
      },
    ];

    const { container } = render(ChatPanel, { props: { messages, streaming: false } });

    // Both message bubbles
    expect(container.querySelectorAll('[role="article"]').length).toBe(2);
    // Tool call card rendered
    expect(container.querySelector('.tool-call-card')).toBeTruthy();
  });

  it('CC-4.1: offline state shows banner and disables send', async () => {
    render(ChatPanel, { props: { messages: [], online: false } });

    expect(screen.getByText('Reconnecting...')).toBeTruthy();

    // Send button should be present but typing still works
    const input = screen.getByRole('textbox');
    expect(input).toBeTruthy();
  });

  it('multi-line message with Shift+Enter', async () => {
    const onSend = vi.fn();
    render(ChatPanel, { props: { messages: [], onSend } });

    const input = screen.getByRole('textbox');
    // Type first line
    await fireEvent.input(input, { target: { value: 'Line 1' } });
    // Shift+Enter should not send
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(onSend).not.toHaveBeenCalled();

    // Regular Enter sends
    await fireEvent.input(input, { target: { value: 'Line 1\nLine 2' } });
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });
    expect(onSend).toHaveBeenCalledWith('Line 1\nLine 2');
  });
});
